import { useEffect, useRef, useState } from "react";
import Matter from "matter-js";
import {
  label,
  radius,
  score,
  tokenUrl,
  type Token,
  type Metric,
} from "./data";
const { Engine, Bodies, Body, Composite } = Matter;
const colors = [
  "#83b9ff",
  "#f8a5d6",
  "#ffcc82",
  "#9de2ca",
  "#b8a4f8",
  "#f8979d",
  "#85dcf3",
];
type Orb = {
  body: Matter.Body;
  token: Token;
  r: number;
  target: number;
  color: string;
  pulse: number;
  image?: HTMLImageElement;
};
export function Arena({
  tokens,
  metric,
  paused,
  demo,
}: {
  tokens: Token[];
  metric: Metric;
  paused: boolean;
  demo: boolean;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const latest = useRef({ tokens, metric, paused, demo });
  latest.current = { tokens, metric, paused, demo };
  const [hover, setHover] = useState<Token | null>(null);
  useEffect(() => {
    const el = canvas.current!;
    const ctx = el.getContext("2d")!;
    const engine = Engine.create({ gravity: { x: 0, y: 0 } });
    const orbs = new Map<string, Orb>();
    let walls: Matter.Body[] = [];
    let width = 0,
      height = 0;
    let raf = 0;
    let last = 0;
    let accumulator = 0;
    let lastTokens: Token[] | null = null;
    let hovered: string | null = null;
    let frameCount = 0;
    let frameStart = performance.now();
    const reduced = matchMedia("(prefers-reduced-motion: reduce)");
    const resize = () => {
      const rect = el.getBoundingClientRect();
      width = rect.width;
      height = rect.height;
      const dpr = Math.min(2, devicePixelRatio || 1);
      el.width = width * dpr;
      el.height = height * dpr;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      Composite.remove(engine.world, walls);
      walls = [
        Bodies.rectangle(width / 2, -35, width + 120, 70, { isStatic: true }),
        Bodies.rectangle(width / 2, height + 35, width + 120, 70, {
          isStatic: true,
        }),
        Bodies.rectangle(-35, height / 2, 70, height + 120, { isStatic: true }),
        Bodies.rectangle(width + 35, height / 2, 70, height + 120, {
          isStatic: true,
        }),
      ];
      Composite.add(engine.world, walls);
      lastTokens = null;
    };
    const observer = new ResizeObserver(resize);
    observer.observe(el);
    resize();
    const sync = () => {
      const state = latest.current;
      const max = Math.max(
        1,
        ...state.tokens.map((t) => score(t, state.metric)),
      );
      const active = new Set(state.tokens.map((t) => t.mint));
      for (const [id, orb] of orbs) {
        if (!active.has(id)) {
          Composite.remove(engine.world, orb.body);
          orbs.delete(id);
        }
      }
      state.tokens.forEach((token, i) => {
        const target = radius(
          score(token, state.metric),
          max,
          width,
          height,
          state.tokens.length,
        );
        const existing = orbs.get(token.mint);
        if (existing) {
          existing.target = target;
          if (existing.token.trade_count !== token.trade_count)
            existing.pulse = 1;
          existing.token = token;
          return;
        }
        const angle = i * 2.399963;
        const spread = Math.sqrt((i + 0.5) / state.tokens.length);
        const x = width / 2 + Math.cos(angle) * spread * width * 0.43;
        const y = height / 2 + Math.sin(angle) * spread * height * 0.43;
        const body = Bodies.circle(x, y, target, {
          restitution: 0.96,
          friction: 0,
          frictionAir: 0,
          slop: 0.05,
        });
        Body.setVelocity(body, {
          x: Math.sin(i * 2.4 + 1) * 0.42,
          y: Math.cos(i * 1.7) * 0.42,
        });
        Composite.add(engine.world, body);
        const orb: Orb = {
          body,
          token,
          r: target,
          target,
          color: colors[i % colors.length],
          pulse: 0,
        };
        if (token.image_url) {
          try {
            const url = new URL(token.image_url);
            if (url.protocol === "https:") {
              const img = new Image();
              img.crossOrigin = "anonymous";
              img.referrerPolicy = "no-referrer";
              img.src = url.href;
              orb.image = img;
            }
          } catch {
            /* Invalid metadata uses text fallback. */
          }
        }
        orbs.set(token.mint, orb);
      });
      if (lastTokens === null) {
        for (let i = 0; i < 100; i++) Engine.update(engine, 1000 / 60);
      }
      lastTokens = state.tokens;
    };
    const render = (now: number) => {
      if (latest.current.tokens !== lastTokens) sync();
      const dt = Math.min(32, now - (last || now));
      last = now;
      const moving = !latest.current.paused && !reduced.matches;
      for (const orb of orbs.values()) {
        const next = orb.r + (orb.target - orb.r) * (1 - Math.exp(-dt / 380));
        if (Math.abs(next - orb.r) > 0.001) {
          Body.scale(orb.body, next / orb.r, next / orb.r);
          orb.r = next;
        }
        const p = orb.body.position;
        const r = orb.r;
        const x = Math.max(r, Math.min(width - r, p.x)),
          y = Math.max(r, Math.min(height - r, p.y));
        if (x !== p.x || y !== p.y) Body.setPosition(orb.body, { x, y });
        if (moving && orb.body.speed < 0.16)
          Body.setVelocity(orb.body, {
            x: Math.sin(now / 5000 + r) * 0.22,
            y: Math.cos(now / 7000 + r) * 0.22,
          });
      }
      // A fixed physics step keeps tab stalls from launching balls through boundaries.
      if (moving) {
        accumulator += dt;
        while (accumulator >= 1000 / 60) {
          Engine.update(engine, 1000 / 60);
          accumulator -= 1000 / 60;
        }
      } else {
        accumulator = 0;
      }
      ctx.clearRect(0, 0, width, height);
      for (const orb of orbs.values()) {
        const { x, y } = orb.body.position;
        const r = orb.r;
        const selected = hovered === orb.token.mint;
        ctx.save();
        ctx.translate(x, y);
        ctx.shadowBlur = selected ? 30 : 12;
        ctx.shadowColor = orb.color + "44";
        const gradient = ctx.createRadialGradient(
          -r * 0.35,
          -r * 0.5,
          0,
          0,
          0,
          r,
        );
        gradient.addColorStop(0, orb.color + "a6");
        gradient.addColorStop(0.45, orb.color + "4d");
        gradient.addColorStop(1, orb.color + "16");
        ctx.fillStyle = gradient;
        ctx.beginPath();
        ctx.arc(0, 0, r, 0, Math.PI * 2);
        ctx.fill();
        ctx.shadowBlur = 0;
        ctx.strokeStyle = orb.color + (selected ? "ed" : "88");
        ctx.lineWidth = selected ? 2 : 1;
        ctx.stroke();
        ctx.beginPath();
        ctx.ellipse(
          -r * 0.21,
          -r * 0.58,
          r * 0.42,
          r * 0.11,
          -0.45,
          0,
          Math.PI * 2,
        );
        ctx.fillStyle = "#ffffff0d";
        ctx.fill();
        orb.pulse = Math.max(0, orb.pulse - dt / 1000);
        if (moving && orb.pulse > 0) {
          ctx.beginPath();
          ctx.arc(0, 0, r + 3 + (1 - orb.pulse) * 7, 0, Math.PI * 2);
          ctx.strokeStyle =
            (orb.token.net_lamports.startsWith("-") ? "#fa91ab" : orb.color) +
            Math.round(orb.pulse * 75)
              .toString(16)
              .padStart(2, "0");
          ctx.stroke();
        }
        const image = orb.image;
        if (image?.complete && image.naturalWidth) {
          ctx.save();
          ctx.beginPath();
          ctx.arc(0, -r * 0.27, r * 0.23, 0, Math.PI * 2);
          ctx.clip();
          ctx.drawImage(image, -r * 0.23, -r * 0.5, r * 0.46, r * 0.46);
          ctx.restore();
        } else if (r > 36) {
          ctx.fillStyle = orb.color;
          ctx.font = `${Math.min(27, r * 0.38)}px sans-serif`;
          ctx.textAlign = "center";
          ctx.fillText(
            ["✦", "●", "✿", "◈", "✧", "◉"][Math.floor(r) % 6],
            0,
            -r * 0.2,
          );
        }
        ctx.textAlign = "center";
        ctx.fillStyle = "#f1f4ff";
        ctx.font = `600 ${Math.max(9, Math.min(17, r * 0.24))}px "Trebuchet MS",sans-serif`;
        ctx.fillText(
          label(orb.token).slice(0, 12),
          0,
          r > 36 ? r * 0.15 : 1,
          r * 1.72,
        );
        if (r > 30) {
          ctx.font = `${Math.max(9, Math.min(12, r * 0.18))}px sans-serif`;
          ctx.fillStyle = orb.color;
          ctx.fillText(
            `${score(orb.token, latest.current.metric).toLocaleString(undefined, { maximumFractionDigits: 1 })} SOL`,
            0,
            r * 0.43,
            r * 1.65,
          );
        }
        ctx.restore();
      }
      frameCount++;
      if (now - frameStart > 2000) {
        el.dataset.fps = String(
          Math.round((frameCount * 1000) / (now - frameStart)),
        );
        frameCount = 0;
        frameStart = now;
      }
      raf = requestAnimationFrame(render);
    };
    raf = requestAnimationFrame(render);
    const hit = (e: PointerEvent | MouseEvent) => {
      const rect = el.getBoundingClientRect();
      return [...orbs.values()]
        .reverse()
        .find(
          (o) =>
            Math.hypot(
              e.clientX - rect.left - o.body.position.x,
              e.clientY - rect.top - o.body.position.y,
            ) < o.r,
        );
    };
    const move = (e: PointerEvent) => {
      const orb = hit(e);
      const id = orb?.token.mint || null;
      if (id !== hovered) {
        hovered = id;
        setHover(orb?.token || null);
      }
      el.style.cursor = orb && !latest.current.demo ? "pointer" : "default";
    };
    const leave = () => {
      hovered = null;
      setHover(null);
    };
    const click = (e: MouseEvent) => {
      const orb = hit(e);
      if (orb && !latest.current.demo) {
        const url = tokenUrl(orb.token);
        if (url) window.open(url, "_blank", "noopener,noreferrer");
      }
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerleave", leave);
    el.addEventListener("click", click);
    return () => {
      cancelAnimationFrame(raf);
      observer.disconnect();
      Composite.clear(engine.world, false);
      Engine.clear(engine);
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerleave", leave);
      el.removeEventListener("click", click);
    };
  }, []);
  return (
    <div className="canvas-wrap">
      <canvas
        ref={canvas}
        aria-label="Animated token arena. The token list below provides accessible links and values."
      />
      {hover && (
        <div className="orb-tooltip">
          <strong>{label(hover)}</strong>
          <span>{hover.trade_count.toLocaleString()} trades</span>
          <span>
            Buy {Number(BigInt(hover.buy_lamports) / 1000000n) / 1000} · Sell{" "}
            {Number(BigInt(hover.sell_lamports) / 1000000n) / 1000} SOL
          </span>
        </div>
      )}
    </div>
  );
}
