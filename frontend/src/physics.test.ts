import { it, expect } from "vitest";
import Matter from "matter-js";
import { radius } from "./data";
import { ball, boundaries, bounceAtEdges, keepMoving } from "./physics";
it("moves freely and bounces off viewport boundaries", () => {
  const engine = Matter.Engine.create({ gravity: { x: 0, y: 0 } });
  const b = ball(250, 100, 20, 0);
  Matter.Composite.add(engine.world, [b, ...boundaries(300, 200)]);
  for (let i = 0; i < 90; i++) Matter.Engine.update(engine, 1000 / 60);
  expect(b.velocity.x).toBeLessThan(0);
  expect(b.position.x).toBeGreaterThan(20);
  expect(b.position.x).toBeLessThan(280);
});
it("transfers momentum between colliding balls", () => {
  const engine = Matter.Engine.create({ gravity: { x: 0, y: 0 } });
  const a = ball(100, 100, 20, 0),
    b = ball(160, 100, 20, Math.PI);
  Matter.Composite.add(engine.world, [a, b]);
  for (let i = 0; i < 30; i++) Matter.Engine.update(engine, 1000 / 60);
  expect(a.velocity.x).toBeLessThan(0);
  expect(b.velocity.x).toBeGreaterThan(0);
  expect(b.position.x - a.position.x).toBeGreaterThan(39);
});

it("keeps slow balls bouncing instead of resting at screen edges", () => {
  const engine = Matter.Engine.create({ gravity: { x: 0, y: 0 } });
  const b = ball(275, 100, 20, 0);
  Matter.Body.setVelocity(b, { x: 0.5, y: 0 });
  Matter.Composite.add(engine.world, [b, ...boundaries(300, 200)]);
  for (let i = 0; i < 60; i++) {
    bounceAtEdges(b, 20, 300, 200);
    Matter.Engine.update(engine, 1000 / 60);
  }
  expect(b.velocity.x).toBeLessThan(-0.4);
  expect(b.position.x).toBeLessThan(270);
});

it("keeps a mixed-size 100-coin arena moving after repeated collisions", () => {
  const width = 459,
    height = 941;
  const values = Array.from({ length: 100 }, (_, i) =>
    i < 6 ? [400, 340, 275, 250, 245, 120][i] : 2 + (i % 12),
  );
  const total = values.reduce((a, b) => a + b, 0);
  const engine = Matter.Engine.create({ gravity: { x: 0, y: 0 } });
  const balls = values.map((v, i) => {
    const r = radius(v, 400, width, height, total),
      angle = i * 2.399963,
      spread = Math.sqrt((i + 0.5) / 100);
    return {
      r,
      body: ball(
        width / 2 + Math.cos(angle) * spread * width * 0.43,
        height / 2 + Math.sin(angle) * spread * height * 0.43,
        r,
        angle,
      ),
    };
  });
  Matter.Composite.add(engine.world, [
    ...balls.map((b) => b.body),
    ...boundaries(width, height),
  ]);
  let previous: { x: number; y: number }[] = [];
  for (let step = 0; step < 600; step++) {
    balls.forEach(({ body, r }, i) => {
      keepMoving(body, i * 2.399963 + step / 900);
      bounceAtEdges(body, r, width, height);
    });
    Matter.Engine.update(engine, 1000 / 60);
    if (step === 479)
      previous = balls.map(({ body }) => ({ ...body.position }));
  }
  const moving = balls.filter(
    ({ body }, i) =>
      Math.hypot(
        body.position.x - previous[i].x,
        body.position.y - previous[i].y,
      ) > 3,
  ).length;
  expect(moving).toBeGreaterThan(80);
});
