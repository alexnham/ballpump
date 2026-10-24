import { z } from "zod";
const unsigned = z.string().regex(/^\d{1,39}$/);
const mint = z.string().regex(/^[1-9A-HJ-NP-Za-km-z]{32,44}$/);
export const tokenSchema = z.object({
  mint,
  symbol: z.string().max(64).nullable(),
  name: z.string().max(256).nullable(),
  image_url: z.string().max(2048).nullable(),
  buy_lamports: unsigned,
  sell_lamports: unsigned,
  net_lamports: z.string().regex(/^-?\d{1,39}$/),
  trade_count: z.number().int().nonnegative(),
  last_trade_ms: z.number().int().nonnegative(),
});
export const snapshotSchema = z.object({
  type: z.literal("leaderboard"),
  window_seconds: z.literal(600),
  as_of_ms: z.number().int(),
  status: z.enum(["live", "demo", "warming", "stale", "degraded"]),
  coverage: z.string(),
  commitment: z.string(),
  indexed_tokens: z.number().int().nonnegative(),
  net_flow: z.array(tokenSchema).max(100),
  buy_volume: z.array(tokenSchema).max(100),
});
export type Token = z.infer<typeof tokenSchema>;
export type Snapshot = z.infer<typeof snapshotSchema>;
export type Metric = "net_flow" | "buy_volume";
export const sol = (value: string) => Number(BigInt(value)) / 1e9; // Display only; accounting stays in integers.
export const label = (t: Token) =>
  t.name?.trim() || t.symbol?.trim() || "Unknown coin";
export function tokenUrl(t: Token) {
  return mint.safeParse(t.mint).success
    ? `https://pump.fun/coin/${t.mint}`
    : undefined;
}
export const score = (t: Token, m: Metric) =>
  sol(m === "net_flow" ? t.net_lamports : t.buy_lamports);
export function radius(
  value: number,
  max: number,
  width: number,
  height: number,
  total: number,
) {
  if (max <= 0 || value <= 0 || width <= 0 || height <= 0) return 0;
  // Fill 50% of the viewport with circles, using one scale for every coin.
  const areaScale = (width * height * 0.5) / (Math.PI * Math.max(total, max));
  const largestRadius = Math.min(width, height) * 0.4;
  const scale = Math.min(areaScale, (largestRadius * largestRadius) / max);
  return Math.sqrt(Math.min(value, max) * scale);
}

const names = [
  "MOCHI",
  "BUBBLE",
  "WIF",
  "POPCAT",
  "BONK",
  "FROG",
  "BONGO",
  "MILO",
  "GIGA",
  "PEPE",
  "PIPPIN",
  "MUMU",
  "NUGGET",
  "GOOSE",
  "BEAN",
  "SLOTH",
  "LUNA",
  "PONK",
  "FIZZ",
  "OTTO",
  "BOP",
  "TACO",
  "MOO",
  "GIZMO",
  "BLOOP",
  "BINK",
  "MEW",
  "PUDGY",
  "JELLY",
  "WOOP",
  "DOT",
  "CHIP",
  "LUCKY",
  "SUSHI",
  "PIP",
  "BEE",
];
// Synthetic symbols and mints; these are never represented as actual token observations.
export function demoSnapshot(time = Date.now()): Snapshot {
  const tokens: Token[] = names.map((name, i) => {
    const buy = Math.round(
      (1800 / (1 + i * 0.35)) * (1 + 0.13 * Math.sin(time / 11000 + i)) * 1e9,
    );
    const sell = Math.round(buy * (0.12 + (i % 7) * 0.09));
    return {
      mint: `${"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"[i + 1]}`.repeat(
        32,
      ),
      symbol: name,
      name: `Demo ${name}`,
      image_url: null,
      buy_lamports: String(buy),
      sell_lamports: String(sell),
      net_lamports: String(buy - sell),
      trade_count: Math.floor((buy / 1e9) * 8),
      last_trade_ms: time - i * 100,
    };
  });
  return {
    type: "leaderboard",
    window_seconds: 600,
    as_of_ms: time,
    status: "demo",
    coverage: "Pump.fun + PumpSwap · SOL/WSOL quotes",
    commitment: "synthetic",
    indexed_tokens: tokens.length,
    net_flow: tokens.sort(
      (a, b) => score(b, "net_flow") - score(a, "net_flow"),
    ),
    buy_volume: [...tokens].sort(
      (a, b) => score(b, "buy_volume") - score(a, "buy_volume"),
    ),
  };
}
