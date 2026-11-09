import { test } from "node:test";
import assert from "node:assert/strict";
import { soundProfile, soundName } from "../src/fx/soundProfile.ts";

test("KB shares its sound choice while console styles and layouts retain native cues", () => {
  for (const choice of ["kb", "sa", "mgsv"]) {
    assert.equal(soundProfile(false, "ps2", "forge", choice), "kb");
    assert.equal(soundProfile(true, "kb", "aurora", choice), "kb");
    for (const console of ["ps2", "ps3", "wii"]) {
      assert.equal(soundProfile(true, "kb", console, choice), console);
      assert.equal(soundProfile(true, console, "forge", choice), console);
    }
  }
  assert.equal(soundProfile(true, "kb", "forge", "mgsv"), "kb");
  assert.equal(soundName("kb", "disc"), "menu_kb_disc");
});
