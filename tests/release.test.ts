import { test } from "node:test";
import assert from "node:assert/strict";
// @ts-expect-error plain JS helpers without type declarations
import { compareVersions, setCargoVersion, setJsonVersion, setLockVersion } from "../tools/release/set-version.mjs";
// @ts-expect-error plain JS helpers without type declarations
import { assetUrl, latestJson } from "../tools/release/latest-json.mjs";
// @ts-expect-error plain JS helpers without type declarations
import { notesFor } from "../tools/release/notes.mjs";
import { readFileSync } from "node:fs";

test("patch notes come from the README's section, one line per change", () => {
  const readme = [
    "# KB",
    "### 0.3.0",
    "- not this one (outside Patch notes)",
    "## Patch notes",
    "Intro text.",
    "### 0.3.10 (2026-11-01)",
    "- other version",
    "### 0.3.1 (2026-10-20)",
    "- New: one",
    "",
    "- Fixed: two",
    "### 0.3.0",
    "- older",
    "## After",
  ].join("\r\n");
  assert.equal(notesFor(readme, "0.3.1"), "- New: one\n- Fixed: two");
  assert.equal(notesFor(readme, "0.3.0"), "- older");
  assert.equal(notesFor(readme, "0.4.0"), null);
  // The real README has notes for the current version.
  const real = readFileSync(new URL("../README.md", import.meta.url), "utf8");
  const conf = JSON.parse(readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"));
  assert.ok(notesFor(real, conf.version), `README has patch notes for ${conf.version}`);
});

test("version bumps touch only the app's own version", () => {
  const conf = '{\n  "productName": "KaneegaBell",\n  "version": "0.2.0",\n  "plugins": { "x": { "version": "9.9.9" } }\n}';
  assert.equal(setJsonVersion(conf, "0.3.0"), conf.replace('"0.2.0"', '"0.3.0"'));

  const cargo = '[package]\nname = "kaneegabell"\nversion = "0.2.0"\n\n[dependencies]\nserde = { version = "1.0" }\n';
  assert.equal(setCargoVersion(cargo, "0.3.0"), cargo.replace('version = "0.2.0"', 'version = "0.3.0"'));

  const lock =
    '{\n  "name": "kaneegabell",\n  "version": "0.2.0",\n  "packages": {\n    "": {\n      "name": "kaneegabell",\n      "version": "0.2.0"\n    },\n    "node_modules/x": { "version": "0.2.0" }\n  }\n}';
  const bumped = setLockVersion(lock, "0.3.0");
  assert.equal((bumped.match(/"0\.3\.0"/g) ?? []).length, 2);
  assert.ok(bumped.includes('"node_modules/x": { "version": "0.2.0" }'), "dependencies untouched");
});

test("versions compare numerically", () => {
  assert.ok(compareVersions("0.10.0", "0.9.9") > 0);
  assert.ok(compareVersions("1.0.0", "1.0.1") < 0);
  assert.equal(compareVersions("0.2.0", "0.2.0"), 0);
});

test("the manifest points at the release's installer with its signature", () => {
  const m = latestJson({
    version: "0.3.0",
    notes: "Fixes",
    signature: "c2lnbmF0dXJl\n",
    fileName: "KaneegaBell_0.3.0_x64-setup.exe",
    pubDate: "2026-10-14T00:00:00Z",
  });
  assert.equal(m.platforms["windows-x86_64"].signature, "c2lnbmF0dXJl");
  assert.equal(
    m.platforms["windows-x86_64"].url,
    "https://github.com/MlghtyZinger/KaneegaBell/releases/download/v0.3.0/KaneegaBell_0.3.0_x64-setup.exe",
  );
  assert.equal(assetUrl("1.0.0", "a b.exe").endsWith("a%20b.exe"), true);
  // On the updates branch, through the contents API (raw bytes for KB's Accept header).
  const api = latestJson({ version: "0.3.0", signature: "s", fileName: "KaneegaBell_0.3.0_x64-setup.exe", contents: true });
  assert.equal(
    api.platforms["windows-x86_64"].url,
    "https://api.github.com/repos/MlghtyZinger/KaneegaBell/contents/KaneegaBell_0.3.0_x64-setup.exe?ref=updates",
  );
  assert.throws(() => latestJson({ version: "1", signature: " ", fileName: "x.exe" }), /signature/);
  assert.throws(() => latestJson({ version: "1", signature: "s", fileName: "x y.exe" }), /spaces/);
});
