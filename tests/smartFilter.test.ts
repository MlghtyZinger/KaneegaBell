// Run with: npm test (Node's built-in runner; Node strips the TypeScript types).
import { test } from "node:test";
import assert from "node:assert/strict";
import { countActive, distinct, emptyRules, matchesRules } from "../src/state/smartFilter.ts";
import type { CollectionRules, GameRow } from "../src/api/index.ts";

const g = (over: Partial<GameRow> = {}): GameRow => ({
  id: 1,
  platformId: "snes",
  title: "Test",
  variant: null,
  region: null,
  favorite: false,
  playSeconds: 0,
  lastPlayed: null,
  isMultidisc: false,
  art: 0,
  artAspect: null,
  ...over,
});

test("an empty filter matches everything", () => {
  assert.equal(matchesRules(g(), emptyRules()), true);
});

test("platform, genre and developer all AND together", () => {
  const r: CollectionRules = { platforms: ["snes"], genres: [3], developers: ["Nintendo"] };
  assert.equal(matchesRules(g({ platformId: "snes", genres: [3, 5], developer: "Nintendo" }), r), true);
  assert.equal(matchesRules(g({ platformId: "snes", genres: [3], developer: "Capcom" }), r), false);
  assert.equal(matchesRules(g({ platformId: "snes", genres: [9], developer: "Nintendo" }), r), false);
});

test("year range and played/unplayed", () => {
  const r: CollectionRules = { yearFrom: 1995, yearTo: 2005, played: "played" };
  assert.equal(matchesRules(g({ year: 2000, playSeconds: 3600 }), r), true);
  assert.equal(matchesRules(g({ year: 1990, playSeconds: 3600 }), r), false);
  assert.equal(matchesRules(g({ year: 2000, playSeconds: 0 }), r), false);
});

test("favorite and play-time bounds", () => {
  const r: CollectionRules = { favorite: true, playFrom: 3600, playTo: 18000 };
  assert.equal(matchesRules(g({ favorite: true, playSeconds: 7200 }), r), true);
  assert.equal(matchesRules(g({ favorite: false, playSeconds: 7200 }), r), false);
  assert.equal(matchesRules(g({ favorite: true, playSeconds: 20000 }), r), false);
});

test("countActive counts set fields", () => {
  assert.equal(countActive({}), 0);
  assert.equal(countActive({ genres: [1], favorite: true }), 2);
  assert.equal(countActive({ played: "unplayed", yearTo: 2000 }), 2);
});

test("distinct trims, dedups and sorts case-insensitively", () => {
  assert.deepEqual(distinct(["Capcom", " nintendo ", "KONAMI", "capcom", null, ""]), ["Capcom", "KONAMI", "nintendo"]);
});
