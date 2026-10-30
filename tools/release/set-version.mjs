// Sets the app version everywhere it's written down:
//   node tools/release/set-version.mjs 0.3.0
// (tauri.conf.json is what the app and installer report; Cargo.toml and
// package.json are kept in step so nothing disagrees.)
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const SEMVER = /^\d+\.\d+\.\d+$/;

export function setJsonVersion(text, version) {
  // Only the top-level "version" (the first one in the file), keeping formatting.
  return text.replace(/("version"\s*:\s*")[^"]*(")/, `$1${version}$2`);
}

export function setCargoVersion(text, version) {
  // The [package] table's version line.
  return text.replace(/(\[package\][^[]*?\nversion\s*=\s*")[^"]*(")/, `$1${version}$2`);
}

export function setLockVersion(text, version) {
  // package-lock.json: the root "version" and the root package entry ("" key).
  let out = setJsonVersion(text, version);
  out = out.replace(/("packages"\s*:\s*\{\s*""\s*:\s*\{[^}]*?"version"\s*:\s*")[^"]*(")/, `$1${version}$2`);
  return out;
}

/** Compares x.y.z versions: negative if a < b. */
export function compareVersions(a, b) {
  const pa = a.split(".").map(Number);
  const pb = b.split(".").map(Number);
  for (let i = 0; i < 3; i++) if (pa[i] !== pb[i]) return pa[i] - pb[i];
  return 0;
}

export function currentVersion(root) {
  return JSON.parse(readFileSync(path.join(root, "src-tauri", "tauri.conf.json"), "utf8")).version;
}

function main() {
  const version = process.argv[2];
  if (!SEMVER.test(version ?? "")) {
    console.error("usage: node tools/release/set-version.mjs <x.y.z>");
    process.exit(2);
  }
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
  const edits = [
    ["src-tauri/tauri.conf.json", setJsonVersion],
    ["src-tauri/Cargo.toml", setCargoVersion],
    ["package.json", setJsonVersion],
    ["package-lock.json", setLockVersion],
  ];
  for (const [file, edit] of edits) {
    const full = path.join(root, file);
    const before = readFileSync(full, "utf8");
    const after = edit(before, version);
    if (after === before && !before.includes(`"${version}"`) && !before.includes(`= "${version}"`)) {
      throw new Error(`couldn't find the version in ${file}`);
    }
    writeFileSync(full, after);
  }
  console.log(`version set to ${version}`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
