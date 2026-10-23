//! Optional display metadata. HTTP lookup never blocks transaction processing.
use crate::model::Snapshot;
use serde::Deserialize;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use tokio::sync::watch;

#[derive(Clone, Default, Deserialize)]
pub struct Metadata {
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub image: Option<String>,
}
pub type Catalog = HashMap<String, Metadata>;

fn text(v: Option<&str>, limit: usize) -> Option<String> {
    v.map(str::trim).filter(|s| !s.is_empty()).map(|s| {
        let mut units = 0;
        s.chars()
            .filter(|c| !c.is_control())
            .take_while(|c| {
                units += c.len_utf16();
                units <= limit
            })
            .collect()
    })
}
fn parse(asset: &serde_json::Value) -> Option<(String, Metadata)> {
    let id = asset.get("id")?.as_str()?.to_owned();
    let content = &asset["content"];
    let image = content["links"]["image"].as_str().and_then(|s| {
        let value = if let Some(cid) = s.strip_prefix("ipfs://") {
            format!("https://ipfs.io/ipfs/{}", cid.trim_start_matches("ipfs/"))
        } else {
            s.to_owned()
        };
        reqwest::Url::parse(&value)
            .ok()
            .filter(|u| u.scheme() == "https" && u.host_str().is_some() && value.len() <= 2048)
            .map(|_| value)
    });
    Some((
        id,
        Metadata {
            name: text(content["metadata"]["name"].as_str(), 256),
            symbol: text(
                content["metadata"]["symbol"]
                    .as_str()
                    .or(asset["token_info"]["symbol"].as_str()),
                64,
            ),
            image,
        },
    ))
}
pub fn enrich(snapshot: &mut Snapshot, catalog: &Catalog) {
    for token in snapshot
        .net_flow
        .iter_mut()
        .chain(snapshot.buy_volume.iter_mut())
    {
        if let Some(meta) = catalog.get(&token.mint) {
            token.name = meta.name.clone();
            token.symbol = meta.symbol.clone();
            token.image_url = meta.image.clone();
        }
    }
}
pub fn start() -> (watch::Sender<Vec<String>>, watch::Receiver<Catalog>) {
    let (requests, mut wanted) = watch::channel(Vec::<String>::new());
    let (updates, catalog) = watch::channel(Catalog::new());
    tokio::spawn(async move {
        let endpoint = std::env::var("HELIUS_RPC_URL")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                format!(
                    "https://mainnet.helius-rpc.com/?api-key={}",
                    std::env::var("LASERSTREAM_API_KEY").unwrap_or_default()
                )
            });
        let Ok(client) = reqwest::Client::builder()
            .timeout(Duration::from_secs(12))
            .build()
        else {
            return;
        };
        let mut cache = Catalog::new();
        let mut fetched: HashMap<String, Instant> = HashMap::new();
        let mut tick = tokio::time::interval(Duration::from_secs(5));
        loop {
            tick.tick().await;
            if wanted.has_changed().is_err() {
                break;
            }
            let visible = wanted.borrow_and_update().clone();
            let mut ids: Vec<_> = visible
                .iter()
                .filter(|id| {
                    fetched
                        .get(*id)
                        .is_none_or(|t| t.elapsed() > Duration::from_secs(300))
                })
                .take(200)
                .cloned()
                .collect();
            ids.sort_unstable();
            ids.dedup();
            if ids.is_empty() {
                continue;
            }
            let response = client.post(&endpoint).json(&serde_json::json!({"jsonrpc":"2.0","id":"ballpump-metadata","method":"getAssetBatch","params":{"ids":ids,"options":{"showFungible":true}}})).send().await;
            // Never log reqwest errors: they can contain the credential-bearing URL.
            let Ok(response) = response else {
                tracing::warn!("token metadata request failed; retrying");
                continue;
            };
            let Ok(response) = response.error_for_status() else {
                tracing::warn!("token metadata HTTP error; retrying");
                continue;
            };
            let Ok(body) = response.json::<serde_json::Value>().await else {
                continue;
            };
            let Some(assets) = body["result"].as_array() else {
                tracing::warn!("token metadata RPC error; retrying");
                continue;
            };
            for id in &ids {
                fetched.insert(id.clone(), Instant::now());
            }
            for asset in assets {
                if let Some((id, meta)) = parse(asset)
                    && ids.contains(&id)
                {
                    cache.insert(id, meta);
                }
            }
            if fetched.len() > 4096 {
                fetched.retain(|id, _| visible.contains(id));
                cache.retain(|id, _| visible.contains(id));
            }
            updates.send_replace(cache.clone());
        }
    });
    (requests, catalog)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn enrichment_keeps_accounting_unchanged_in_both_rankings() {
        let mut snapshot = crate::aggregate::Window::default().snapshot("live".into(), false);
        let token = crate::model::Token {
            mint: "mint".into(),
            buy_lamports: "18446744073709551615".into(),
            net_lamports: "42".into(),
            ..Default::default()
        };
        snapshot.net_flow.push(token.clone());
        snapshot.buy_volume.push(token);
        let mut catalog = Catalog::new();
        catalog.insert(
            "mint".into(),
            Metadata {
                name: Some("Coin".into()),
                symbol: Some("COIN".into()),
                image: None,
            },
        );
        enrich(&mut snapshot, &catalog);
        for t in snapshot.net_flow.iter().chain(snapshot.buy_volume.iter()) {
            assert_eq!(t.name.as_deref(), Some("Coin"));
            assert_eq!(t.buy_lamports, "18446744073709551615");
            assert_eq!(t.net_lamports, "42");
        }
        assert!(
            text(Some(&"🐸".repeat(200)), 256)
                .unwrap()
                .encode_utf16()
                .count()
                <= 256
        );
    }
    #[test]
    fn reads_das_fungible_metadata_and_rejects_unsafe_images() {
        let a = serde_json::json!({"id":"mint", "content":{"metadata":{"name":" Coin ","symbol":"COIN"},"links":{"image":"ipfs://abc"}}});
        let (_, m) = parse(&a).unwrap();
        assert_eq!(m.name.as_deref(), Some("Coin"));
        assert_eq!(m.image.as_deref(), Some("https://ipfs.io/ipfs/abc"));
        let mut a = a;
        a["content"]["links"]["image"] = "javascript:alert(1)".into();
        assert!(parse(&a).unwrap().1.image.is_none());
        assert!(parse(&serde_json::Value::Null).is_none());
    }
}
