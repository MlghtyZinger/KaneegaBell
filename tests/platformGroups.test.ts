import { test } from "node:test";
import assert from "node:assert/strict";
import { platformMaker, platformRailRows } from "../src/state/platformGroups.ts";

test("maker folding keeps fixed tabs and only exposes open groups' children", () => {
  const tabs = ["all", "favorites", "collections", "ps2", "wii", "gba", "xbox"];
  const rows = platformRailRows(tabs, ["ps2", "wii", "gba", "xbox"], ["nintendo"]);
  assert.deepEqual(rows.map((r) => r.id), ["all", "favorites", "collections", "maker:nintendo", "wii", "gba", "maker:sony", "maker:microsoft"]);
  assert.deepEqual(tabs, ["all", "favorites", "collections", "ps2", "wii", "gba", "xbox"]);
  const ps = rows.find((r) => r.id === "maker:sony");
  assert.ok(ps?.kind === "maker");
  assert.deepEqual(ps.platforms, ["ps2"]);
});

test("empty makers disappear and future platforms remain reachable under Other", () => {
  assert.deepEqual(platformRailRows(["all"], [], []).map((r) => r.id), ["all"]);
  assert.deepEqual(platformRailRows(["all", "future"], ["future"], ["other"]).map((r) => r.id), ["all", "maker:other", "future"]);
  assert.equal(platformMaker("future"), "other");
  assert.equal(platformMaker("psp"), "sony");
  assert.equal(platformMaker("saturn"), "sega");
});
