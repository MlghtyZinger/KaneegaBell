import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { defineConfig, type Plugin } from "vite";
import solid from "vite-plugin-solid";
// @ts-expect-error plain JS helper without type declarations
import { allNotes } from "./tools/release/notes.mjs";

/**
 * `virtual:patch-notes`: the README's Patch notes section as data, built into
 * the app for the "What's new" window (loaded only when it opens).
 */
function patchNotes(): Plugin {
  const id = "virtual:patch-notes";
  const resolved = "\0" + id;
  const readme = fileURLToPath(new URL("./README.md", import.meta.url));
  return {
    name: "kb-patch-notes",
    resolveId: (source) => (source === id ? resolved : null),
    load(source) {
      if (source !== resolved) return null;
      this.addWatchFile(readme);
      return `export default ${JSON.stringify(allNotes(readFileSync(readme, "utf8")))};`;
    },
  };
}

// Tauri expects a fixed dev port and doesn't need Vite's screen clearing.
export default defineConfig({
  plugins: [solid(), patchNotes()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    // perf-lib: generated libraries and screenshots (tools/perf), not app code.
    watch: { ignored: ["**/src-tauri/**", "**/perf-lib/**"] },
  },
  build: {
    // WebView2 is Chromium; no legacy transpilation needed.
    target: "es2022",
    sourcemap: false,
  },
});
