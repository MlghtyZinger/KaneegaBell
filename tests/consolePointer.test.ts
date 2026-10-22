import { test } from "node:test";
import assert from "node:assert/strict";
import { pointerDelta } from "../src/views/consolePointer.ts";

test("Wii pointer ignores drift, accelerates smoothly and caps stale samples", () => {
  for (const axis of [-.2, 0, .1, .2, NaN]) assert.equal(pointerDelta(axis, 16, 1920), 0);
  assert.ok(pointerDelta(.5, 16, 1920) > 0);
  assert.ok(pointerDelta(1, 16, 1920) > pointerDelta(.5, 16, 1920));
  assert.equal(pointerDelta(-1, 16, 1920), -pointerDelta(1, 16, 1920));
  assert.equal(pointerDelta(1, 3000, 1920), pointerDelta(1, 40, 1920));
  assert.equal(pointerDelta(1, 16, Infinity), 0);
  assert.equal(pointerDelta(1, -1, 1920), 0);
});
