import { test } from "node:test";
import assert from "node:assert/strict";
import { channelWindow, showcaseScale, showcaseWindow, xmbWindow } from "../src/views/tvShowcase.ts";

test("Showcase keeps a five-item window containing the selected game throughout a large library", () => {
  for (const count of [0, 1, 3, 5, 5000]) {
    for (const focused of [0, Math.floor(count / 2), Math.max(0, count - 1)]) {
      const visible = showcaseWindow(focused, count);
      assert.equal(visible.length, Math.min(5, count));
      assert.ok(visible.every((i) => i >= 0 && i < count));
      if (count) assert.ok(visible.includes(focused));
    }
  }
});

test("Showcase fits the entire case at 720p, 1080p and 4K, including wide and tall art", () => {
  for (const [width, height] of [[280, 480], [420, 680], [840, 1360]]) {
    for (const aspect of [0.4, 0.8, 1.4, 1.8, 2.5]) {
      const scale = showcaseScale(width, height, 134, 190, aspect);
      const h = Math.sqrt(134 * 190 * aspect);
      assert.ok(h * scale <= height * 0.8 + 0.001);
      assert.ok(h / aspect * scale <= width * 0.8 + 0.001);
    }
  }
});

test("Wii channel pages contain the selection and never render over twelve items", () => {
  for (const count of [0, 1, 11, 12, 13, 5000]) {
    for (let selected = 0; selected < Math.max(1, count); selected++) {
      const page = channelWindow(selected, count);
      assert.ok(page.length <= 12);
      assert.ok(page.every((i) => i >= 0 && i < count));
      if (count) assert.ok(page.includes(selected));
      if (page.length) assert.equal(page[0] % 12, 0);
    }
  }
});

test("XMB keeps a fixed selection row with one previous and four following items", () => {
  for (const count of [0, 1, 4, 5000]) {
    for (const focused of [0, Math.floor(count / 2), Math.max(0, count - 1)]) {
      const visible = xmbWindow(focused, count);
      assert.ok(visible.length <= 6);
      assert.ok(visible.every((index) => index >= 0 && index < count && index - focused >= -1 && index - focused <= 4));
      if (count) assert.ok(visible.includes(focused));
    }
  }
});
