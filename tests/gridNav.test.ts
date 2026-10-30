// Run with: npm test (Node's built-in runner; Node strips the TypeScript types).
import { test } from "node:test";
import assert from "node:assert/strict";
import { buildRows, gridMove } from "../src/components/gridNav.ts";

// 3 SNES, 5 GBA, 1 GC games, 4 columns.
const platforms = ["snes", "snes", "snes", "gba", "gba", "gba", "gba", "gba", "gc"];
const grouped = buildRows(platforms.length, 4, (i) => platforms[i]);

test("plain rows fill up to the column count", () => {
  const r = buildRows(9, 4);
  assert.deepEqual(r.start, [0, 4, 8]);
  assert.deepEqual(r.len, [4, 4, 1]);
  assert.deepEqual(r.header, [null, null, null]);
});

test("grouped rows break at every platform change, with a header", () => {
  assert.deepEqual(grouped.start, [0, 3, 7, 8]);
  assert.deepEqual(grouped.len, [3, 4, 1, 1]);
  assert.deepEqual(grouped.header, ["snes", "gba", null, "gc"]);
  assert.deepEqual(grouped.row, [0, 0, 0, 1, 1, 1, 1, 2, 3]);
});

test("left/right walk the list across groups", () => {
  assert.equal(gridMove(grouped, 2, "right"), 3); // last SNES -> first GBA
  assert.equal(gridMove(grouped, 3, "left"), 2);
  assert.equal(gridMove(grouped, 0, "left"), 0);
  assert.equal(gridMove(grouped, 8, "right"), 8);
});

test("up/down keep the column, clamped to shorter rows", () => {
  assert.equal(gridMove(grouped, 1, "down"), 4); // SNES #2 -> GBA #2
  assert.equal(gridMove(grouped, 6, "up"), 2); // GBA #4 -> last SNES (row of 3)
  assert.equal(gridMove(grouped, 6, "down"), 7); // -> the lone GBA in row 3
  assert.equal(gridMove(grouped, 7, "down"), 8); // -> GC
  assert.equal(gridMove(grouped, 8, "down"), 8);
  assert.equal(gridMove(grouped, 0, "up"), 0);
});

test("empty list", () => {
  const r = buildRows(0, 4, () => "x");
  assert.deepEqual(r.start, []);
  assert.equal(gridMove(r, 0, "down"), 0);
});
