// Integration check: real HTTP/WS service, current snapshots on reconnect, and process restart.
import { spawn } from "node:child_process";
import assert from "node:assert/strict";
const root = new URL("../", import.meta.url).pathname;
const port = process.env.SMOKE_PORT || "3099";
let child;
async function start() {
  child = spawn(root + "backend/target/debug/ballpump", [], {
    cwd: root,
    env: {
      ...process.env,
      SOURCE: "demo",
      BIND_ADDR: `127.0.0.1:${port}`,
      RUST_LOG: "error",
    },
    stdio: "ignore",
  });
  for (let i = 0; i < 100; i++) {
    try {
      if ((await fetch(`http://127.0.0.1:${port}/health`)).ok) return;
    } catch {}
    await new Promise((r) => setTimeout(r, 100));
  }
  throw Error("Server did not start");
}
async function stop() {
  if (child && child.exitCode === null) {
    const ended = new Promise((r) => child.once("exit", r));
    child.kill("SIGINT");
    await ended;
  }
}
function snapshot() {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
    const timeout = setTimeout(() => {
      ws.close();
      reject(Error("WS timeout"));
    }, 5000);
    ws.onmessage = ({ data }) => {
      clearTimeout(timeout);
      ws.close();
      resolve(JSON.parse(data));
    };
    ws.onerror = reject;
  });
}
try {
  await start();
  const a = await snapshot();
  assert.equal(a.type, "leaderboard");
  assert.equal(a.status, "demo");
  assert.equal(a.window_seconds, 600);
  await new Promise((r) => setTimeout(r, 1200));
  const b = await snapshot();
  assert(b.as_of_ms > a.as_of_ms);
  assert(b.indexed_tokens > 0);
  assert(
    b.net_flow.every(
      (t) => typeof t.buy_lamports === "string" && BigInt(t.net_lamports) > 0n,
    ),
  );
  const metrics = await (
    await fetch(`http://127.0.0.1:${port}/metrics`)
  ).json();
  assert(metrics.decoded_swaps > 0);
  assert.equal(metrics.dropped_events, 0);
  await stop();
  await start();
  const c = await snapshot();
  assert.equal(c.status, "demo");
  assert(c.as_of_ms >= b.as_of_ms);
  console.log(
    "PASS HTTP, WebSocket, reconnect full snapshot, integer contract, process restart",
  );
} finally {
  await stop();
}
