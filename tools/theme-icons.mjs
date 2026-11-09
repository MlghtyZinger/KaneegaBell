// Makes the bull icon in each built-in style's colors, for the window and taskbar
// icon (src-tauri/icons/themes/<style>.png). Run after changing a style's icon
// colors:  node tools/theme-icons.mjs
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, relative, resolve, isAbsolute } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const master = readFileSync(join(root, "src-tauri/icons/icon-master.svg"), "utf8");

// The master's colors (Ember), and each style's replacement for them.
const SLOTS = { t1: "#d2462a", t2: "#5a140a", line: "#f3e6c4", h1: "#fbf4e0", h2: "#e3cf98", head: "#17181d", eyes: "#f7e4a0", muzzle: "#3b3d48", ring: "#efb446" };
const STYLES = {
  forge: { t1: "#ed783f", t2: "#382013", line: "#f6ead8", h1: "#fff0d7", h2: "#d3ad75", eyes: "#ffbd80", ring: "#e8b65e" },
  starlight: { t1: "#72609f", t2: "#171126", line: "#f1edff", h1: "#f6edff", h2: "#bca7ef", eyes: "#e1d5ff", ring: "#e9ca82" },
  obsidian: { t1: "#35474a", t2: "#101416", line: "#77e5cf", h1: "#e7eef0", h2: "#91b6b0", eyes: "#77e5cf", ring: "#77e5cf" },
  aurora: { t1: "#377a63", t2: "#071c1c", line: "#b0f5d7", h1: "#e2faf3", h2: "#87d8b4", eyes: "#9aecc9", ring: "#9aecc9" },
  chrome: { t1: "#f0f5fc", t2: "#9eb2cd", line: "#172c48", h1: "#ffffff", h2: "#b4c4dc", eyes: "#69aaff", ring: "#2457a6" },
  ps3: { t1: "#276b93", t2: "#071a33", line: "#e8f8ff", h1: "#ffffff", h2: "#9bd8f0", eyes: "#b9f4ff", ring: "#69c6ed" },
  wii: { t1: "#ffffff", t2: "#cadce8", line: "#007eae", h1: "#ffffff", h2: "#a9d8ee", head: "#4a606e", eyes: "#7ce0ff", muzzle: "#8097a5", ring: "#007ba6" },
  ps2: { t1: "#174276", t2: "#02040a", line: "#9ac7ff", h1: "#e5efff", h2: "#729acb", eyes: "#d1e6ff", ring: "#659ee8" },
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
    if (id === "forge") {
      const defaultDir = join(work, "default");
      execFileSync(process.execPath, [join(root, "node_modules/@tauri-apps/cli/tauri.js"), "icon", file, "-o", defaultDir], { stdio: "ignore" });
      for (const name of ["icon.ico", "icon.png", "32x32.png", "128x128.png", "128x128@2x.png"]) copyFileSync(join(defaultDir, name), join(root, "src-tauri/icons", name));
    }
    console.log(`${id}.png`);
  }
} finally {
  const inside = relative(resolve(tmpdir()), resolve(work));
  if (!inside || inside.startsWith("..") || isAbsolute(inside)) throw Error("Icon scratch directory escaped the temporary folder");
  rmSync(work, { recursive: true, force: true });
}
