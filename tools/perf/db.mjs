// Read-only facts about a library for perf.ps1: node db.mjs <library.db> count|game
import { DatabaseSync } from "node:sqlite";
const [file, what] = process.argv.slice(2);
const db = new DatabaseSync(file, { readOnly: true });
if (what === "count") {
  console.log(db.prepare("SELECT COUNT(*) AS n FROM games WHERE missing = 0").get().n);
} else if (what === "game") {
  // A PS2 game, when the generated stand-in emulator is configured.
  const g = db
    .prepare(
      "SELECT g.id FROM games g JOIN emulators e ON e.id = 'pcsx2' WHERE g.platform_id = 'ps2' AND e.exe_path IS NOT NULL LIMIT 1",
    )
    .get();
  console.log(g ? g.id : "");
}
