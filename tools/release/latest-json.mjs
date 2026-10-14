// Writes the updater manifest (latest.json) for a release:
//   node tools/release/latest-json.mjs <version> <installer.exe> <notes.md> <out.json> [--contents]
// The signature is the contents of the installer's .sig file (made by the
// signed build). Without --contents the installer link is the release's public
// download link (KB 0.2.2 and older read this, while the repo was public). With
// --contents it's the installer stored on the repo's `updates` branch, read through
// GitHub's contents API: KB 0.2.3+ sends "Accept: application/vnd.github.raw+json"
// (and its access code) for both the manifest and the installer, and that API
// answers it with the file's raw bytes, public or private.
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const REPO = "MlghtyZinger/KaneegaBell";

export function assetUrl(version, fileName) {
  return `https://github.com/${REPO}/releases/download/v${version}/${encodeURIComponent(fileName)}`;
}

/** A file on the `updates` branch through GitHub's contents API. */
export function contentsUrl(fileName) {
  if (/[\s/\\?#]/.test(fileName)) throw new Error(`bad file name: ${fileName}`);
  return `https://api.github.com/repos/${REPO}/contents/${encodeURIComponent(fileName)}?ref=updates`;
}

export function latestJson({ version, notes, signature, fileName, pubDate, contents }) {
  if (!signature || !signature.trim()) throw new Error("the installer's signature is empty");
  if (/\s/.test(fileName)) throw new Error("installer file names can't contain spaces (GitHub renames them)");
  return {
    version,
    notes: notes ?? "",
    pub_date: pubDate,
    platforms: {
      "windows-x86_64": {
        signature: signature.trim(),
        url: contents ? contentsUrl(fileName) : assetUrl(version, fileName),
      },
    },
  };
}

function main() {
  const [version, installer, notesFile, out, mode] = process.argv.slice(2);
  if (!out) {
    console.error("usage: node tools/release/latest-json.mjs <version> <installer.exe> <notes.md> <out.json> [--contents]");
    process.exit(2);
  }
  const manifest = latestJson({
    version,
    notes: readFileSync(notesFile, "utf8").trim(),
    signature: readFileSync(`${installer}.sig`, "utf8"),
    fileName: path.basename(installer),
    pubDate: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
    contents: mode === "--contents",
  });
  writeFileSync(out, JSON.stringify(manifest, null, 2) + "\n");
  console.log(`wrote ${out}`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
