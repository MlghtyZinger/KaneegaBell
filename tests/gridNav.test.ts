// Run with: npm test (Node's built-in runner; Node strips the TypeScript types).
import { test } from "node:test";
import assert from "node:assert/strict";
import { stackLayout } from "../src/components/gridNav.ts";

const GAP = 10;

test("games read left to right, row after row", () => {
  const p = stackLayout([100, 60, 140, 60, 100, 60, 40], 3, GAP, 1000);
  assert.deepEqual(p.col, [0, 1, 2, 0, 1, 2, 0]);
});

test("each card sits right under the one above it in its column (no gaps)", () => {
  const p = stackLayout([100, 60, 140, 60, 100, 60], 3, GAP, 1000);
  assert.deepEqual(p.y, [0, 0, 0, 110, 70, 150]);
  assert.equal(p.total, 210); // column 2: 140 + 10 + 60
});

test("columns that drift too far apart are lined up again at the next row", () => {
  // Column 0 gets tall covers, the others short ones: after row 2 the spread
  // (300 vs 100) passes the 150 limit, so row 3 starts level.
  const h = [200, 50, 50, 200, 50, 50, 80, 80, 80];
  const p = stackLayout(h, 3, 0, 150);
  assert.deepEqual(p.y.slice(6), [400, 400, 400]);
  // Below the limit nothing moves.
  const loose = stackLayout(h, 3, 0, 1000);
  assert.deepEqual(loose.y.slice(6), [400, 100, 100]);
});

test("same-shape covers make a plain grid", () => {
  const p = stackLayout(Array(7).fill(100), 3, GAP, 50);
  assert.deepEqual(p.y, [0, 0, 0, 110, 110, 110, 220]);
});

test("empty and single-column lists", () => {
  assert.equal(stackLayout([], 4, GAP, 50).total, 0);
  assert.deepEqual(stackLayout([10, 20], 1, GAP, 50).y, [0, 20]);
});
