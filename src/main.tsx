import { render } from "solid-js/web";
import App from "./App";
import "./styles/tokens.css";
import "./styles/base.css";
import { initTheme } from "./state/theme";

async function start() {
  // Outside Tauri (plain browser during development) serve fake data instead.
  if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
    (await import("./dev/mockIpc")).installMockIpc();
    // Test hook: simulate controller actions in the mock (no Rust gamepad thread).
    const { dispatchFromGamepad } = await import("./input/actions");
    Object.assign(window, { __pad: dispatchFromGamepad });
  }
  initTheme();
  render(() => <App />, document.getElementById("root")!);
}

void start();
