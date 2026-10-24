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
import { ball, boundaries, bounceAtEdges, keepMoving } from "./physics";
const { Engine, Body, Composite, Constraint } = Matter;
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
  imageUrl?: string | null;
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
    const resize = () => {
      const rect = el.getBoundingClientRect();
      width = rect.width;
      height = rect.height;
      const dpr = Math.min(2, devicePixelRatio || 1);
      el.width = width * dpr;
      el.height = height * dpr;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      Composite.remove(engine.world, walls);
      walls = boundaries(width, height);
      for (const orb of orbs.values()) {
        Body.setPosition(orb.body, {
          x: Math.max(orb.r, Math.min(width - orb.r, orb.body.position.x)),
          y: Math.max(orb.r, Math.min(height - orb.r, orb.body.position.y)),
        });
      }
      Composite.add(engine.world, walls);
      lastTokens = null;
    };
    const observer = new ResizeObserver(resize);
    observer.observe(el);
    resize();
    const loadImage = (orb: Orb, token: Token) => {
      if (orb.imageUrl === token.image_url) return;
      orb.imageUrl = token.image_url;
      orb.image = undefined;
      if (!token.image_url) return;
      try {
        const url = new URL(token.image_url);
        if (url.protocol !== "https:") return;
        const image = new Image();
        image.referrerPolicy = "no-referrer";
        image.src = url.href;
        orb.image = image;
      } catch {
        /* Missing or invalid artwork uses the name fallback. */
      }
    };
    const sync = () => {
      const state = latest.current;
      const max = Math.max(
        0,
        ...state.tokens.map((t) => score(t, state.metric)),
      );
      const total = state.tokens.reduce(
        (sum, t) => sum + Math.max(0, score(t, state.metric)),
        0,
      );
      const active = new Set(state.tokens.map((t) => t.mint));
      for (const [id, orb] of orbs) {
        if (!active.has(id)) {
          if (drag?.orb === orb) {
            Composite.remove(engine.world, drag.constraint);
            drag = null;
            suppressClick = true;
          }
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
          total,
        );
        const existing = orbs.get(token.mint);
        if (existing) {
          existing.target = target;
          if (existing.token.trade_count !== token.trade_count)
            existing.pulse = 1;
          loadImage(existing, token);
          existing.token = token;
          return;
        }
        const angle = i * 2.399963;
        const spread = Math.sqrt((i + 0.5) / state.tokens.length);
        const x = width / 2 + Math.cos(angle) * spread * width * 0.43;
        const y = height / 2 + Math.sin(angle) * spread * height * 0.43;
        const body = ball(
          Math.max(target, Math.min(width - target, x)),
          Math.max(target, Math.min(height - target, y)),
          target,
          i * 2.4 + 1,
        );
        Composite.add(engine.world, body);
        const orb: Orb = {
          body,
          token,
          r: target,
          target,
          color: colors[i % colors.length],
          pulse: 0,
        };
        loadImage(orb, token);
        orbs.set(token.mint, orb);
      });
      if (lastTokens === null) {
        for (let i = 0; i < 80; i++) {
          for (const orb of orbs.values())
            bounceAtEdges(orb.body, orb.r, width, height);
          Engine.update(engine, 1000 / 60);
        }
      }
      lastTokens = state.tokens;
    };
    const render = (now: number) => {
      if (latest.current.tokens !== lastTokens) sync();
      const dt = Math.min(32, now - (last || now));
      last = now;
      const moving = !latest.current.paused;
      let motionIndex = 0;
      for (const orb of orbs.values()) {
        if (moving && drag?.orb !== orb)
          keepMoving(orb.body, motionIndex * 2.399963 + now / 15000);
        motionIndex++;
        const next = orb.r + (orb.target - orb.r) * (1 - Math.exp(-dt / 380));
        if (Math.abs(next - orb.r) > 0.001) {
          Body.scale(orb.body, next / orb.r, next / orb.r);
          orb.r = next;
        }
      }
      // A fixed physics step keeps tab stalls from launching balls through boundaries.
      if (moving) {
        accumulator += dt;
        while (accumulator >= 1000 / 60) {
          for (const orb of orbs.values())
            bounceAtEdges(orb.body, orb.r, width, height);
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
        const hasImage = image?.complete && image.naturalWidth;
        if (hasImage) {
          ctx.save();
          ctx.beginPath();
          ctx.arc(0, 0, r - 1, 0, Math.PI * 2);
          ctx.clip();
          const scale = Math.max(
            (2 * r) / image.naturalWidth,
            (2 * r) / image.naturalHeight,
          );
          ctx.drawImage(
            image,
            (-image.naturalWidth * scale) / 2,
            (-image.naturalHeight * scale) / 2,
            image.naturalWidth * scale,
            image.naturalHeight * scale,
          );
          const shade = ctx.createLinearGradient(0, -r, 0, r);
          shade.addColorStop(0, "#10162400");
          shade.addColorStop(0.4, "#10162415");
          shade.addColorStop(1, "#101624ed");
          ctx.fillStyle = shade;
          ctx.fillRect(-r, -r, 2 * r, 2 * r);
          ctx.restore();
        } else if (r > 36) {
          ctx.fillStyle = orb.color;
          ctx.textAlign = "center";
          ctx.font = `600 ${Math.min(32, r * 0.5)}px "Space Grotesk",sans-serif`;
          ctx.fillText(
            (orb.token.symbol || label(orb.token)).slice(0, 1).toUpperCase(),
            0,
            -r * 0.18,
          );
        }
        ctx.textAlign = "center";
        ctx.fillStyle = "#f1f4ff";
        ctx.shadowColor = "#000";
        ctx.shadowBlur = 5;
        ctx.font = `600 ${Math.max(9, Math.min(17, r * 0.24))}px "Space Grotesk",sans-serif`;
        ctx.fillText(
          label(orb.token).length > 20
            ? label(orb.token).slice(0, 19) + "…"
            : label(orb.token),
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
        el.dataset.balls = String(orbs.size);
        el.dataset.images = String(
          [...orbs.values()].filter(
            (o) => o.image?.complete && o.image.naturalWidth,
          ).length,
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
    let drag: {
      orb: Orb;
      constraint: Matter.Constraint;
      startX: number;
      startY: number;
      moved: boolean;
      x: number;
      y: number;
      time: number;
      vx: number;
      vy: number;
    } | null = null;
    let suppressClick = false;
    const down = (e: PointerEvent) => {
      const orb = hit(e);
      if (!orb || drag || !e.isPrimary || e.button !== 0) return;
      const rect = el.getBoundingClientRect();
      const point = { x: e.clientX - rect.left, y: e.clientY - rect.top };
      const constraint = Constraint.create({
        pointA: point,
        bodyB: orb.body,
        pointB: {
          x: point.x - orb.body.position.x,
          y: point.y - orb.body.position.y,
        },
        stiffness: 0.12,
        damping: 0.1,
        length: 0,
      });
      Composite.add(engine.world, constraint);
      drag = {
        orb,
        constraint,
        startX: e.clientX,
        startY: e.clientY,
        moved: false,
        x: point.x,
        y: point.y,
        time: e.timeStamp,
        vx: 0,
        vy: 0,
      };
      suppressClick = false;
      el.setPointerCapture(e.pointerId);
    };
    const up = (e: PointerEvent) => {
      if (!drag) return;
      Composite.remove(engine.world, drag.constraint);
      suppressClick = drag.moved || e.type === "pointercancel";
      if (e.timeStamp - drag.time > 100) {
        drag.vx = 0;
        drag.vy = 0;
      }
      if (drag.moved)
        Body.setVelocity(drag.orb.body, {
          x: Math.max(-12, Math.min(12, drag.vx)),
          y: Math.max(-12, Math.min(12, drag.vy)),
        });
      drag = null;
      if (el.hasPointerCapture(e.pointerId))
        el.releasePointerCapture(e.pointerId);
    };
    const move = (e: PointerEvent) => {
      if (drag) {
        const rect = el.getBoundingClientRect();
        const x = e.clientX - rect.left,
          y = e.clientY - rect.top;
        const dt = Math.max(1, e.timeStamp - drag.time);
        drag.vx = ((x - drag.x) * 16.667) / dt;
        drag.vy = ((y - drag.y) * 16.667) / dt;
        drag.constraint.pointA = { x, y };
        drag.moved ||=
          Math.hypot(e.clientX - drag.startX, e.clientY - drag.startY) > 6;
        if (latest.current.paused)
          Body.setPosition(drag.orb.body, {
            x: Math.max(drag.orb.r, Math.min(width - drag.orb.r, x)),
            y: Math.max(drag.orb.r, Math.min(height - drag.orb.r, y)),
          });
        drag.x = x;
        drag.y = y;
        drag.time = e.timeStamp;
        el.style.cursor = "grabbing";
        return;
      }
      const orb = hit(e);
      const id = orb?.token.mint || null;
      if (id !== hovered) {
        hovered = id;
        setHover(orb?.token || null);
      }
      el.style.cursor = orb ? "grab" : "default";
    };
    const leave = () => {
      hovered = null;
      setHover(null);
    };
    const click = (e: MouseEvent) => {
      if (suppressClick) {
        suppressClick = false;
        return;
      }
      const orb = hit(e);
      if (orb && !latest.current.demo) {
        const url = tokenUrl(orb.token);
        if (url) window.open(url, "_blank", "noopener,noreferrer");
      }
    };
    el.addEventListener("pointerdown", down);
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", up);
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerleave", leave);
    el.addEventListener("click", click);
    return () => {
      cancelAnimationFrame(raf);
      observer.disconnect();
      Composite.clear(engine.world, false);
      Engine.clear(engine);
      el.removeEventListener("pointerdown", down);
      el.removeEventListener("pointerup", up);
      el.removeEventListener("pointercancel", up);
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerleave", leave);
      el.removeEventListener("click", click);
    };
  }, []);
  return (
    <div className="canvas-wrap">
      <canvas
        ref={canvas}
        aria-label="Animated token arena. Open the token list for accessible links and values. Drag balls to throw them."
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
