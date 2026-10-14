import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

// Tauri expects a fixed dev port and doesn't need Vite's screen clearing.
export default defineConfig({
  plugins: [solid()],
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
