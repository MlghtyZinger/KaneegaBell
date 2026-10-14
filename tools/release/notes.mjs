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

/**
 * Every version in the Patch notes section, newest first, for KB's "What's
 * new" window: [{ version, date, changes: [{ kind, text }] }]. `kind` is the
 * line's "New:" / "Fixed:" / "Changed:" / "Improved:" label, lowercased ("" if none).
 */
export function allNotes(readme) {
  const lines = readme.replace(/\r\n/g, "\n").split("\n");
  const start = lines.findIndex((l) => /^##\s+Patch notes\s*$/i.test(l));
  if (start < 0) return [];
  const out = [];
  for (let i = start + 1; i < lines.length && !/^##?\s/.test(lines[i]); i++) {
    const h = /^###\s+v?(\d+\.\d+\.\d+)\s*(?:\(([^)]*)\))?/.exec(lines[i]);
    if (h) {
      out.push({ version: h[1], date: h[2]?.trim() || null, changes: [] });
      continue;
    }
    const line = lines[i].trim();
    if (!line.startsWith("- ") || out.length === 0) continue;
    const m = /^(New|Fixed|Changed|Improved):\s*(.*)$/i.exec(line.slice(2));
    // The label becomes a tag, so the text starts a sentence of its own.
    // (Markdown code marks are dropped: the window shows plain text.)
    const cap = (s) => (s.charAt(0).toUpperCase() + s.slice(1)).replace(/`/g, "");
    out[out.length - 1].changes.push(m ? { kind: m[1].toLowerCase(), text: cap(m[2]) } : { kind: "", text: cap(line.slice(2)) });
  }
  return out;
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
