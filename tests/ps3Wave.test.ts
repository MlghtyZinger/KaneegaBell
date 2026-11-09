import { test } from "node:test";
import assert from "node:assert/strict";
import { xmbHue, xmbWave } from "../src/fx/ps3Wave.ts";

test("XMB color stays fixed for a local date and changes across midnight, month and year boundaries", () => {
  for (const day of [new Date(2026, 9, 30), new Date(2026, 9, 31), new Date(2026, 11, 31)]) {
    const late = new Date(day.getFullYear(), day.getMonth(), day.getDate(), 23, 59);
    const next = new Date(day.getFullYear(), day.getMonth(), day.getDate() + 1);
    assert.equal(xmbHue(day), xmbHue(late));
    assert.notEqual(xmbHue(day), xmbHue(next));
  }
});

test("XMB ribbons deform visibly within seconds and stay inside the scene", () => {
  const changes = [0, .25, .5, .75, 1].map(x => xmbWave(0, x, 3).y - xmbWave(0, x, 0).y);
  assert.ok(Math.max(...changes) - Math.min(...changes) > .1, "shape changes, not just uniform translation");
  for (let band = 0; band < 7; band++) for (let t = 0; t < 60; t += 3) for (let x = 0; x <= 1; x += .1) {
    const p = xmbWave(band, x, t);
    assert.ok(p.y > .2 && p.y + p.width < .8);
    assert.ok(p.width > 0 && p.width < .05);
  }
});
