// Takes a version's patch notes from the README's "Patch notes" section:
//   node tools/release/notes.mjs <version> <out.md>
// Each version is a "### X.Y.Z (date)" heading followed by one "- " line per
// change. The lines become the release's changelog (shown in KB's update window).
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

/** The change lines under `### <version>` in the Patch notes section, or null. */
export function notesFor(readme, version) {
  const lines = readme.replace(/\r\n/g, "\n").split("\n");
  const start = lines.findIndex((l) => /^##\s+Patch notes\s*$/i.test(l));
  if (start < 0) return null;
  const heading = new RegExp(`^###\\s+v?${version.replace(/\./g, "\\.")}(?![\\d.])`);
  let i = lines.findIndex((l, n) => n > start && heading.test(l));
  if (i < 0) return null;
  const out = [];
  for (i++; i < lines.length && !/^#{1,3}\s/.test(lines[i]); i++) {
    const line = lines[i].trim();
    if (line.startsWith("- ")) out.push(line);
  }
  return out.length ? out.join("\n") : null;
}

function main() {
  const [version, out] = process.argv.slice(2);
  if (!out) {
    console.error("usage: node tools/release/notes.mjs <version> <out.md>");
    process.exit(2);
  }
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
  const notes = notesFor(readFileSync(path.join(root, "README.md"), "utf8"), version);
  if (!notes) {
    console.error(`README.md has no "### ${version}" lines under "## Patch notes"`);
    process.exit(1);
  }
  writeFileSync(out, notes + "\n");
  console.log(`notes for ${version}:\n${notes}`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
