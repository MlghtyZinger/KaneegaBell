import { render } from "solid-js/web";
import App from "./App";
import "./styles/tokens.css";
import "./styles/base.css";

async function start() {
  // Outside Tauri (plain browser during development) serve fake data instead.
  if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
    (await import("./dev/mockIpc")).installMockIpc();
  }
  render(() => <App />, document.getElementById("root")!);
}

void start();
