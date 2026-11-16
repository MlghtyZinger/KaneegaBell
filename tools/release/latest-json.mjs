// Writes the updater manifest (latest.json) for a release:
//   node tools/release/latest-json.mjs <version> <installer.exe> <notes.md> <out.json> [assetId]
// The signature is the contents of the installer's .sig file (made by the
// signed build). Without an asset id the installer link is the release's
// public download link (KB 0.2.2 and older read this, while the repo is public);
// with one it's the GitHub API link, which also works with an access code once
// the repo is private (KB 0.2.3 and newer, via the updates branch).
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const REPO = "MlghtyZinger/KaneegaBell";

export function assetUrl(version, fileName) {
  return `https://github.com/${REPO}/releases/download/v${version}/${encodeURIComponent(fileName)}`;
}

/** The installer through GitHub's API (works for private repos with a token). */
export function apiAssetUrl(assetId) {
  if (!/^\d+$/.test(String(assetId))) throw new Error(`bad asset id: ${assetId}`);
  return `https://api.github.com/repos/${REPO}/releases/assets/${assetId}`;
}

export function latestJson({ version, notes, signature, fileName, pubDate, assetId }) {
  if (!signature || !signature.trim()) throw new Error("the installer's signature is empty");
  if (/\s/.test(fileName)) throw new Error("installer file names can't contain spaces (GitHub renames them)");
  return {
    version,
    notes: notes ?? "",
    pub_date: pubDate,
    platforms: {
      "windows-x86_64": {
        signature: signature.trim(),
        url: assetId ? apiAssetUrl(assetId) : assetUrl(version, fileName),
      },
    },
  };
}

function main() {
  const [version, installer, notesFile, out, assetId] = process.argv.slice(2);
  if (!out) {
    console.error("usage: node tools/release/latest-json.mjs <version> <installer.exe> <notes.md> <out.json> [assetId]");
    process.exit(2);
  }
  const manifest = latestJson({
    version,
    notes: readFileSync(notesFile, "utf8").trim(),
    signature: readFileSync(`${installer}.sig`, "utf8"),
    fileName: path.basename(installer),
    pubDate: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
    assetId,
  });
  writeFileSync(out, JSON.stringify(manifest, null, 2) + "\n");
  console.log(`wrote ${out}`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
