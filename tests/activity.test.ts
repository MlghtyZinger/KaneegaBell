import test from "node:test";
import assert from "node:assert/strict";
import { progressValue, countText } from "../src/state/activityModel.ts";
test("activity keeps unknown totals indeterminate and never treats progress as completion", () => {
  assert.equal(progressValue({ done: 40, total: 0, status: "running" }), undefined);
  assert.equal(progressValue({ done: 0, total: 8, status: "queued" }), undefined);
  assert.equal(progressValue({ done: 7, total: 8, status: "running" }), 87.5);
  assert.equal(progressValue({ done: 5, total: 0, status: "failed" }), undefined);
  assert.equal(progressValue({ done: 0, total: 0, status: "done" }), 100);
  assert.equal(progressValue({ done: NaN, total: 20, status: "running" }), undefined);
});
test("progress clamps invalid counts and resets between phases", () => {
  assert.equal(progressValue({ done: -1, total: 10, status: "running" }), 0);
  assert.equal(progressValue({ done: 20, total: 10, status: "running" }), 100);
  assert.equal(progressValue({ done: 1, total: 10, status: "running" }), 10);
  assert.equal(countText(0, 0), "Preparing...");
  assert.equal(countText(30, 20, "icons"), "20 / 20 icons");
});
