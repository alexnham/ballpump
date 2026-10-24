import Matter from "matter-js";
const { Bodies, Body } = Matter;
export function ball(x: number, y: number, r: number, angle: number) {
  const body = Bodies.circle(x, y, r, {
    restitution: 1,
    inertia: Infinity,
    friction: 0,
    frictionStatic: 0,
    frictionAir: 0,
    slop: 0.01,
  });
  Body.setVelocity(body, {
    x: Math.cos(angle) * 3.2,
    y: Math.sin(angle) * 3.2,
  });
  return body;
}
export function boundaries(width: number, height: number) {
  const options = { isStatic: true, restitution: 1, friction: 0 };
  return [
    Bodies.rectangle(width / 2, -50, width + 200, 100, options),
    Bodies.rectangle(width / 2, height + 50, width + 200, 100, options),
    Bodies.rectangle(-50, height / 2, 100, height + 200, options),
    Bodies.rectangle(width + 50, height / 2, 100, height + 200, options),
  ];
}
// Resolve viewport edges analytically so Matter's resting threshold cannot pin slow balls.
export function bounceAtEdges(
  body: Matter.Body,
  r: number,
  width: number,
  height: number,
) {
  const { x, y } = body.position;
  let { x: vx, y: vy } = body.velocity;
  if ((vx < 0 && x + vx < r) || (vx > 0 && x + vx > width - r)) vx = -vx;
  if ((vy < 0 && y + vy < r) || (vy > 0 && y + vy > height - r)) vy = -vy;
  Body.setVelocity(body, { x: vx, y: vy });
  if (x < r || x > width - r || y < r || y > height - r)
    Body.setPosition(body, {
      x: Math.max(r, Math.min(width - r, x)),
      y: Math.max(r, Math.min(height - r, y)),
    });
}

// A gentle drift replenishes motion lost to Matter's resting-contact solver.
export function keepMoving(body: Matter.Body, angle: number) {
  if (body.speed < 0.3)
    Body.setVelocity(body, {
      x: Math.cos(angle) * 1.8,
      y: Math.sin(angle) * 1.8,
    });
}
