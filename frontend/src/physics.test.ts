import { it, expect } from "vitest";
import Matter from "matter-js";
import { ball, boundaries, bounceAtEdges } from "./physics";
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
