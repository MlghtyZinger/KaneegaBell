import test from "node:test";
import assert from "node:assert/strict";
import { PC_LAUNCHERS, isLauncher, launcherName } from "../src/state/launchers.ts";
import { platformMaker } from "../src/state/platformGroups.ts";
test("PC launcher identities remain separate from emulated Xbox consoles", () => {
  assert.equal(new Set(PC_LAUNCHERS.map((p) => p.id)).size, 8);
  for (const p of PC_LAUNCHERS) { assert.ok(isLauncher(p.id)); assert.equal(launcherName(p.id), p.name); assert.equal(platformMaker(p.id), "pc"); }
  assert.equal(isLauncher("xbox"), false); assert.equal(isLauncher("xbox360"), false);
  assert.equal(platformMaker("xbox"), "microsoft");
});
