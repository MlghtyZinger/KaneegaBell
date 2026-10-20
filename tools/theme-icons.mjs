// Makes the bull icon in each built-in style's colors, for the window and taskbar
// icon (src-tauri/icons/themes/<style>.png). Run after changing a style's icon
// colors:  node tools/theme-icons.mjs
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const master = readFileSync(join(root, "src-tauri/icons/icon-master.svg"), "utf8");

// The master's colors (Ember), and each style's replacement for them.
const SLOTS = { t1: "#d2462a", t2: "#5a140a", line: "#f3e6c4", h1: "#fbf4e0", h2: "#e3cf98", head: "#17181d", eyes: "#f7e4a0", muzzle: "#3b3d48", ring: "#efb446" };
const STYLES = {
  ember: {},
  midnight: { t1: "#3a7be0", t2: "#0b1a3a", line: "#e8eef8", head: "#15171d", eyes: "#ffd75e", muzzle: "#363a48", ring: "#ffd75e" },
  daylight: { t1: "#f6f8fb", t2: "#c3ccd9", line: "#14171c", h2: "#d9c48a", ring: "#c76a00" },
  contrast: { t1: "#000000", t2: "#000000", line: "#00ffd0", h1: "#ffffff", h2: "#e6e6e6", head: "#000000", eyes: "#ffffff", muzzle: "#2a2a2a", ring: "#00ffd0" },
  moss: { t1: "#4f9a6c", t2: "#0f2a1b", line: "#e8efe9", head: "#141a17", eyes: "#f1e08a", muzzle: "#34403a", ring: "#f1e08a" },
  storm: { t1: "#2f9ea3", t2: "#0b2a30", line: "#e9eef3", head: "#14181e", eyes: "#ff9f4a", muzzle: "#353d48", ring: "#ff9f4a" },
};

const work = mkdtempSync(join(tmpdir(), "kb-icons-"));
const out = join(root, "src-tauri/icons/themes");
mkdirSync(out, { recursive: true });
try {
  for (const [id, colors] of Object.entries(STYLES)) {
    let svg = master;
    for (const [slot, from] of Object.entries(SLOTS)) svg = svg.replaceAll(`"${from}"`, `"${colors[slot] ?? from}"`);
    const file = join(work, `${id}.svg`);
    writeFileSync(file, svg);
    execFileSync(process.execPath, [join(root, "node_modules/@tauri-apps/cli/tauri.js"), "icon", file, "-p", "64", "-o", join(work, id)], { stdio: "ignore" });
    copyFileSync(join(work, id, "64x64.png"), join(out, `${id}.png`));
    console.log(`${id}.png`);
  }
} finally {
  rmSync(work, { recursive: true, force: true });
}
