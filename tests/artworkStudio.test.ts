import { test } from "node:test";
import assert from "node:assert/strict";
import { applyCall } from "../src/state/artworkStudio.ts";

const candidate = { source: "sgdb" as const, name: "42", url: "https://x/42.png", preview: null, label: "SteamGridDB" };

test("a chosen option sets that slot from its own source", () => {
  assert.deepEqual(applyCall("back", { op: "option", c: candidate }), { op: "set", source: "sgdb", name: "42", url: "https://x/42.png" });
});

test("an image file sets the slot from the file", () => {
  assert.deepEqual(applyCall("spine", { op: "file", path: "C:\\art\\s.png" }), { op: "set", source: "file", name: "C:\\art\\s.png", url: "C:\\art\\s.png" });
});

test("removing the cover uses the cover call, other slots use the extras call", () => {
  assert.deepEqual(applyCall("cover", { op: "remove" }), { op: "removeCover" });
  assert.deepEqual(applyCall("logo", { op: "remove" }), { op: "removeExtra", kind: "logo" });
  assert.deepEqual(applyCall("icon", { op: "remove" }), { op: "removeExtra", kind: "icon" });
});

test("the icon can be the cover", () => {
  assert.deepEqual(applyCall("icon", { op: "useCover" }), { op: "useCover" });
});
