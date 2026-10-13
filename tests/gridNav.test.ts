// Run with: npm test (Node's built-in runner; Node strips the TypeScript types).
import { test } from "node:test";
import assert from "node:assert/strict";
import { stackedMove, type NavDir, type StackedLayout } from "../src/components/gridNav.ts";

const GAP = 10;

/** Places cards like the grid does: each under the shortest column. */
function stack(heights: number[], cols: number): StackedLayout {
  const y: number[] = [];
  const col: number[] = [];
  const columns: number[][] = Array.from({ length: cols }, () => []);
  const bottom = new Array(cols).fill(0);
  heights.forEach((h, i) => {
    let k = 0;
    for (let j = 1; j < cols; j++) if (bottom[j] < bottom[k] - 0.5) k = j;
    y.push(bottom[k]);
    bottom[k] += h + GAP;
    col.push(k);
    columns[k].push(i);
  });
  return { y, h: heights, col, columns, gap: GAP };
}

/** Follows a sequence of moves the way the grid does (carrying the line). */
function walk(L: StackedLayout, start: number, dirs: NavDir[]): number[] {
  let i = start;
  let line: number | null = null;
  const seen = [];
  for (const d of dirs) {
    ({ index: i, line } = stackedMove(L, i, d, line));
    seen.push(i);
  }
  return seen;
}

// Card spans (top..bottom), gap 10:
//  col0          col1          col2
//  0   0..100    1   0..60     2   0..140
//                3  70..130
//  4 110..210    5 140..200    6 150..190
//                8 210..270    7 200..300
const L = stack([100, 60, 140, 60, 100, 60, 40, 100, 60], 3);

test("the test layout is what the comment says", () => {
  assert.deepEqual(L.columns, [[0, 4], [1, 3, 5, 8], [2, 6, 7]]);
  assert.deepEqual([...L.y], [0, 0, 0, 70, 110, 140, 150, 200, 210]);
});

test("up and down stay in the column", () => {
  assert.deepEqual(walk(L, 1, ["down", "down", "down", "down"]), [3, 5, 8, 8]);
  assert.deepEqual(walk(L, 8, ["up", "up", "up", "up"]), [5, 3, 1, 1]);
});

test("right then left comes back to the same card", () => {
  // From 4 (y 110..210, center 160): col1 card at 160 is 5 (y 140..200).
  assert.deepEqual(walk(L, 4, ["right", "left"]), [5, 4]);
  assert.deepEqual(walk(L, 0, ["right", "right", "left", "left"]), [1, 2, 1, 0]);
});

test("a run of sideways moves keeps to one line instead of drifting", () => {
  // Line 160 (center of 4): col1 -> 5, col2 -> 6 (y 150..190).
  assert.deepEqual(walk(L, 4, ["right", "right"]), [5, 6]);
  // And back along the same line, even though 6's own center is 170.
  assert.deepEqual(walk(L, 4, ["right", "right", "left", "left"]), [5, 6, 5, 4]);
});

test("right past the last column wraps to the next line, left wraps back", () => {
  // 2's center is 70; the first col0 card below that line is 4.
  assert.deepEqual(walk(L, 2, ["right"]), [4]);
  // Left from 4 (center 160): the last column's card above the line is 2.
  assert.deepEqual(walk(L, 4, ["left"]), [2]);
  // Holding right keeps moving down to the bottom of the grid, even after the
  // first column has run out (after 6 it continues in col1 with 8, then 7).
  const path = walk(L, 0, Array(10).fill("right"));
  assert.deepEqual(path.slice(0, 7), [1, 2, 4, 5, 6, 8, 7]);
  assert.equal(path[path.length - 1], 7, "stops at the last line");
  // Holding left from there climbs back up to the top.
  assert.equal(walk(L, 7, Array(10).fill("left")).at(-1), 0);
});

test("edges with nowhere to go stay put", () => {
  assert.deepEqual(walk(L, 0, ["left"]), [0]);
  const one = stack([100, 100], 1);
  assert.deepEqual(walk(one, 0, ["right", "left"]), [0, 0]);
});
