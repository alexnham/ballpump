import { StrictMode, useState } from "react";
import { createRoot } from "react-dom/client";
import { Arena } from "./Arena";
import { useFeed } from "./useFeed";
import { label, score, tokenUrl, type Metric } from "./data";
import "./style.css";
function App() {
  const [demo, setDemo] = useState(
    new URLSearchParams(location.search).get("demo") === "1",
  );
  const [metric, setMetric] = useState<Metric>("net_flow");
  const [paused, setPaused] = useState(false);
  const [list, setList] = useState(false);
  const { snapshot, connection } = useFeed(demo);
  const tokens = snapshot?.[metric].slice(0, 100) || [];
  const synthetic = demo || snapshot?.status === "demo";
  const status = synthetic
    ? "Demo"
    : connection !== "Connected"
      ? connection
      : snapshot?.status || "Connecting";
  const toggleDemo = () => {
    const next = !demo;
    setDemo(next);
    const url = new URL(location.href);
    if (next) url.searchParams.set("demo", "1");
    else url.searchParams.delete("demo");
    history.replaceState({}, "", url);
  };
  const note = synthetic
    ? "Synthetic activity. No real trades."
    : status === "live"
      ? "Finalized activity · replay coverage unverified"
      : status === "warming"
        ? "Building the trailing window; coverage incomplete"
        : status === "degraded"
          ? "Decode or recovery gaps; totals may be incomplete"
          : "Feed unavailable or behind; gaps are not zero volume";
  return (
    <main>
      <Arena
        tokens={tokens}
        metric={metric}
        paused={paused}
        demo={!!synthetic}
      />
      <div className="hud">
        <header>
          <a className="brand" href="/" aria-label="BallPump home">
            <span className="brand-mark">◒</span>ballpump
            <span className="beta">beta</span>
          </a>
          <div className="header-actions">
            <span className={"status " + status.toLowerCase()}>
              <i />
              {status}
            </span>
            <button onClick={toggleDemo}>
              {demo ? "Connect feed" : "Demo"}
            </button>
          </div>
        </header>
        <div className="controls">
          <div className="metric-tabs" role="group" aria-label="Ranking metric">
            <button
              aria-pressed={metric === "net_flow"}
              onClick={() => setMetric("net_flow")}
            >
              10m Net Buy Flow
            </button>
            <button
              aria-pressed={metric === "buy_volume"}
              onClick={() => setMetric("buy_volume")}
            >
              10m Buy Volume
            </button>
          </div>
          <button
            className="pause"
            aria-pressed={paused}
            onClick={() => {
              setPaused(!paused);
            }}
            aria-label={paused ? "Resume animation" : "Pause animation"}
          >
            {paused ? "▶ Resume" : "Ⅱ Pause"}
          </button>
        </div>
      </div>
      {!tokens.length && (
        <div className="empty">
          <span>◎</span>
          <h1>Waiting for the flow</h1>
          <p>Coins appear as qualifying trades arrive.</p>
          {!demo && <button onClick={toggleDemo}>Explore demo</button>}
        </div>
      )}
      <div className="bottom-hud">
        <div className="arena-caption">
          <strong>{tokens.length} coins in motion</strong>
          <span>
            Ball area ={" "}
            {metric === "net_flow"
              ? "positive net buy flow"
              : "gross buy volume"}{" "}
            in SOL
          </span>
          <span className="interaction-hint">
            {synthetic
              ? "Drag & throw · Demo coins"
              : "Drag & throw · Click to open Pump.fun"}
          </span>
        </div>
        <div className="coverage">
          <span>Pump.fun + PumpSwap · Rolling 10 minutes</span>
          <span className={"coverage-note " + status.toLowerCase()}>
            {note}
          </span>
          <small>Flow does not prove price growth or legitimacy.</small>
        </div>
        <button
          className="list-toggle"
          aria-expanded={list}
          aria-controls="rankings"
          onClick={() => setList(!list)}
        >
          {list ? "Close list ×" : "Token list ↗"}
        </button>
      </div>
      {list && (
        <section
          id="rankings"
          className="token-list"
          aria-label="Token rankings"
        >
          <div className="list-head">
            <div>
              <h2>The current lineup</h2>
              <p>Rolling 10 minutes · SOL</p>
            </div>
            <button
              onClick={() => setList(false)}
              aria-label="Close token list"
            >
              ×
            </button>
          </div>
          {tokens.map((t, i) => (
            <div className="token-row" key={t.mint}>
              <span className="rank">{i + 1}</span>
              <div>
                {synthetic ? (
                  <strong>{label(t)}</strong>
                ) : (
                  <a
                    href={tokenUrl(t)}
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    {label(t)} ↗
                  </a>
                )}
                <small>
                  {t.symbol || "Metadata pending"} ·{" "}
                  {t.trade_count.toLocaleString()} trades
                </small>
              </div>
              <strong>
                {score(t, metric).toLocaleString(undefined, {
                  maximumFractionDigits: 1,
                })}
                <small>SOL</small>
              </strong>
            </div>
          ))}
        </section>
      )}
    </main>
  );
}
createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
