import { describe, it, expect } from "vitest";
import {
  label,
  demoSnapshot,
  radius,
  snapshotSchema,
  score,
  sol,
  tokenUrl,
} from "./data";
describe("feed contract and sizing", () => {
  it("validates a standalone demo and ranks both metrics", () => {
    const d = snapshotSchema.parse(demoSnapshot(1700000000000));
    for (const metric of ["net_flow", "buy_volume"] as const) {
      expect(d[metric]).toHaveLength(36);
      for (let i = 1; i < d[metric].length; i++)
        expect(score(d[metric][i - 1], metric)).toBeGreaterThanOrEqual(
          score(d[metric][i], metric),
        );
    }
  });
  it("keeps amounts lossless until display and rejects malformed payloads", () => {
    const d = demoSnapshot();
    d.net_flow[0].buy_lamports = "18446744073709551615";
    expect(snapshotSchema.parse(d).net_flow[0].buy_lamports).toBe(
      "18446744073709551615",
    );
    expect(sol("1000000001")).toBe(1.000000001);
    d.net_flow[0].net_lamports = "NaN";
    expect(snapshotSchema.safeParse(d).success).toBe(false);
  });
  it("bounds outliers and adapts to mobile", () => {
    expect(radius(1e20, 1e20, 1200, 600, 1e20)).toBeLessThanOrEqual(600 * 0.4);
    expect(radius(1, 100, 350, 470, 50)).toBeGreaterThan(0);
    expect(radius(25, 100, 1200, 600, 50)).toBeLessThan(
      radius(100, 100, 1200, 600, 50),
    );
  });
  it("does not turn malicious metadata into a URL", () => {
    const t = demoSnapshot().net_flow[0];
    expect(tokenUrl({ ...t, mint: "javascript:alert(1)" })).toBeUndefined();
    expect(tokenUrl(t)).toMatch(/^https:\/\/pump.fun\/coin\//);
  });
});

it("uses coin names, then tickers, without showing mint IDs as names", () => {
  const t = demoSnapshot().net_flow[0];
  expect(label({ ...t, name: "My Coin", symbol: "MC" })).toBe("My Coin");
  expect(label({ ...t, name: null, symbol: "MC" })).toBe("MC");
  expect(label({ ...t, name: null, symbol: null })).toBe("Unknown coin");
});

it("keeps ball areas proportional, including tiny and sub-SOL flows", () => {
  for (const [width, height] of [
    [1200, 720],
    [319, 941],
  ]) {
    for (const max of [100, 0.1]) {
      const largest = radius(max, max, width, height, 50);
      const half = radius(max / 2, max, width, height, 50);
      const quarter = radius(max / 4, max, width, height, 50);
      expect(half ** 2 / largest ** 2).toBeCloseTo(0.5, 12);
      expect(quarter / largest).toBeCloseTo(0.5, 12);
      expect(radius(max / 10000, max, width, height, 50) / largest).toBeCloseTo(
        0.01,
        12,
      );
    }
  }
  expect(radius(0, 100, 1200, 720, 50)).toBe(0);
  expect(radius(-1, 100, 1200, 720, 50)).toBe(0);
  expect(radius(0, 0, 1200, 720, 50)).toBe(0);
});

it("packs 100 proportionally sized coins with room for collision motion", () => {
  const values = Array.from({ length: 100 }, (_, i) => 100 - i);
  const total = values.reduce((a, b) => a + b, 0);
  for (const [width, height] of [
    [1280, 720],
    [319, 941],
  ]) {
    const radii = values.map((v) => radius(v, 100, width, height, total));
    const area = radii.reduce((sum, r) => sum + Math.PI * r * r, 0);
    expect(area / (width * height)).toBeCloseTo(0.5, 10);
    expect(radii[0] ** 2 / radii[99] ** 2).toBeCloseTo(100, 10);
    expect(Math.max(...radii)).toBeLessThan(Math.min(width, height) / 2);
  }
});
