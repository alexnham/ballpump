use axum::{
    Json, Router,
    extract::{
        State,
        ws::{Message, WebSocketUpgrade},
    },
    response::IntoResponse,
    routing::get,
};
use ballpump::{
    aggregate::{Insert, Window},
    ingest,
    metrics::Metrics,
    model::*,
};
use futures_util::{SinkExt, StreamExt};
use std::{
    sync::{Arc, atomic::Ordering::Relaxed},
    time::Duration,
};
use tokio::sync::{mpsc, watch};
use tower_http::services::{ServeDir, ServeFile};
#[derive(Clone)]
struct App {
    snapshots: watch::Receiver<String>,
    metrics: Arc<Metrics>,
}
async fn websocket(State(state): State<App>, upgrade: WebSocketUpgrade) -> impl IntoResponse {
    upgrade.on_upgrade(move |socket| async move {
        let (mut sink,mut source)=socket.split();let mut rx=state.snapshots;
        let initial=rx.borrow_and_update().clone();
        if sink.send(Message::Text(initial.into())).await.is_err(){return;}
        loop {tokio::select! {
            changed=rx.changed()=>{
                if changed.is_err(){break;}
                let msg=rx.borrow_and_update().clone();
                if tokio::time::timeout(Duration::from_secs(5),sink.send(Message::Text(msg.into()))).await.is_ok_and(|r|r.is_ok()) == false{break;}
            },
            msg=source.next()=>match msg {
                Some(Ok(Message::Ping(p)))=>{if sink.send(Message::Pong(p)).await.is_err(){break;}},
                Some(Ok(Message::Close(_)))|None|Some(Err(_))=>break,
                _=>{}
            }
        }}
    })
}
async fn checkpoint(path: &str, slot: u64) -> anyhow::Result<()> {
    use tokio::io::AsyncWriteExt;
    let temp = format!("{path}.tmp");
    let mut f = tokio::fs::File::create(&temp).await?;
    f.write_all(slot.to_string().as_bytes()).await?;
    f.sync_all().await?;
    tokio::fs::rename(&temp, path).await?;
    Ok(())
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ballpump=info".into()),
        )
        .init();
    let mode = std::env::var("SOURCE").unwrap_or("demo".into());
    anyhow::ensure!(
        mode == "demo" || mode == "live",
        "SOURCE must be demo or live"
    );
    let demo = mode == "demo";
    if !demo {
        for key in ["LASERSTREAM_API_KEY", "LASERSTREAM_ENDPOINT"] {
            anyhow::ensure!(
                std::env::var(key).is_ok_and(|v| !v.is_empty()),
                "{key} is required for live mode"
            );
        }
    }
    let m = Arc::new(Metrics::default());
    let mut window = Window::default();
    window.advance(now_ms());
    let initial = serde_json::to_string(
        &window.snapshot(if demo { "demo" } else { "warming" }.into(), demo),
    )?;
    let (publisher, subscriber) = watch::channel(initial);
    let (tx, mut rx) = mpsc::channel::<ingest::Batch>(1024);
    let path = std::env::var("CHECKPOINT_PATH").unwrap_or(".ballpump-cursor".into());
    if demo {
        tokio::spawn(ingest::demo(tx, m.clone()));
    } else {
        let (raw_tx, raw_rx) = mpsc::channel(128);
        tokio::spawn(ingest::decode_loop(raw_rx, tx, m.clone()));
        let metrics = m.clone();
        let cursor = path.clone();
        tokio::spawn(async move {
            if ingest::live(raw_tx, metrics, cursor).await.is_err() {
                tracing::error!("live source stopped");
            }
        });
    }
    let state = App {
        snapshots: subscriber,
        metrics: m.clone(),
    };
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        let mut warm_until = now_ms() + WINDOW_MS;
        let mut slot = 0;
        let mut saved = 0;
        let mut previous_received = 0;
        let mut previous_swaps = 0;
        loop {
            tokio::select! {
                Some(batch)=rx.recv()=>{
                    let start=std::time::Instant::now();let now=now_ms();
                    for trade in batch.trades {match window.insert(trade,now) {
                        Insert::Duplicate=>{m.duplicates.fetch_add(1,Relaxed);},
                        Insert::Expired=>{m.expired.fetch_add(1,Relaxed);},
                        Insert::Future=>{m.future.fetch_add(1,Relaxed);m.decode_degraded_until.store(now+WINDOW_MS,Relaxed);},
                        Insert::Added=>{}
                    }}
                    slot=slot.max(batch.slot);m.aggregation_us.store(start.elapsed().as_micros() as u64,Relaxed);
                },
                _=tick.tick()=>{
                    let now=now_ms();window.advance(now);
                    let stale=now-m.last_message_ms.load(Relaxed)>15_000 || now-m.latest_chain_ms.load(Relaxed)>60_000;
                    if stale {warm_until=now+WINDOW_MS;}
                    let status=if demo {"demo"}else if stale {"stale"}else if now<m.decode_degraded_until.load(Relaxed){"degraded"}else if now<warm_until {"warming"}else{"live"};
                    let snapshot=window.snapshot(status.into(),demo);
                    if let Ok(json)=serde_json::to_string(&snapshot){m.snapshot_bytes.store(json.len() as u64,Relaxed);publisher.send_replace(json);}
                    let received=m.received.load(Relaxed);let swaps=m.swaps.load(Relaxed);
                    tracing::info!(transactions_per_second=received-previous_received,swaps_per_second=swaps-previous_swaps,indexed_tokens=snapshot.indexed_tokens,status,metrics=?m.report(),"snapshot published");
                    previous_received=received;previous_swaps=swaps;
                    if !demo && slot>saved {match checkpoint(&path,slot).await {Ok(())=>saved=slot,Err(e)=>{tracing::error!(error=%e,"checkpoint failed");m.decode_degraded_until.store(now+WINDOW_MS,Relaxed);}}}
                }
            }
        }
    });
    let assets = std::env::var("FRONTEND_DIST").unwrap_or("frontend/dist".into());
    let app = Router::new()
        .route("/ws", get(websocket))
        .route(
            "/metrics",
            get(|State(s): State<App>| async move { Json(s.metrics.report()) }),
        )
        .route(
            "/health",
            get(|State(s): State<App>| async move {
                axum::response::Response::builder()
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(s.snapshots.borrow().clone()))
                    .unwrap()
            }),
        )
        .fallback_service(
            ServeDir::new(&assets)
                .not_found_service(ServeFile::new(format!("{assets}/index.html"))),
        )
        .with_state(state);
    let bind = std::env::var("BIND_ADDR").unwrap_or("127.0.0.1:3000".into());
    tracing::info!(%bind,%mode,"BallPump ready");
    axum::serve(tokio::net::TcpListener::bind(bind).await?, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
