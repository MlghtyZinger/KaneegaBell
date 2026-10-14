// Writes the updater manifest (latest.json) for a release:
//   node tools/release/latest-json.mjs <version> <installer.exe> <notes.md> <out.json>
// The signature is the contents of the installer's .sig file (made by the
// signed build). The app downloads the installer from this release's assets.
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const REPO = "MlghtyZinger/KaneegaBell";

export function assetUrl(version, fileName) {
  return `https://github.com/${REPO}/releases/download/v${version}/${encodeURIComponent(fileName)}`;
}

export function latestJson({ version, notes, signature, fileName, pubDate }) {
  if (!signature || !signature.trim()) throw new Error("the installer's signature is empty");
  if (/\s/.test(fileName)) throw new Error("installer file names can't contain spaces (GitHub renames them)");
  return {
    version,
    notes: notes ?? "",
    pub_date: pubDate,
    platforms: {
      "windows-x86_64": { signature: signature.trim(), url: assetUrl(version, fileName) },
    },
  };
}

function main() {
  const [version, installer, notesFile, out] = process.argv.slice(2);
  if (!out) {
    console.error("usage: node tools/release/latest-json.mjs <version> <installer.exe> <notes.md> <out.json>");
    process.exit(2);
  }
  const manifest = latestJson({
    version,
    notes: readFileSync(notesFile, "utf8").trim(),
    signature: readFileSync(`${installer}.sig`, "utf8"),
    fileName: path.basename(installer),
    pubDate: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
  });
  writeFileSync(out, JSON.stringify(manifest, null, 2) + "\n");
  console.log(`wrote ${out}`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
