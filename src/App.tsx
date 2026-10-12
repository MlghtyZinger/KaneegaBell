import { createSignal, Match, onCleanup, onMount, Switch } from "solid-js";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { events } from "./api";
import { library } from "./state/library";
import { session } from "./state/session";
import { dispatch, installKeyboard, type Action } from "./input/actions";
import { LibraryView } from "./views/Library";
import { SettingsView } from "./views/Settings";
import { NowPlaying } from "./views/NowPlaying";
import "./styles/app.css";

type View = "library" | "settings";

export default function App() {
  const [view, setView] = createSignal<View>("library");
  const win = getCurrentWindow();

  onCleanup(installKeyboard());

  const onF11 = (e: KeyboardEvent) => {
    if (e.key === "F11") {
      e.preventDefault();
      void win.isFullscreen().then((fs) => win.setFullscreen(!fs));
    }
  };
  window.addEventListener("keydown", onF11);
  onCleanup(() => window.removeEventListener("keydown", onF11));

  // The window opens visible with the app's background color (no white flash).
  // It isn't started hidden: on Windows, WebView2 sometimes never paints a
  // webview whose window is shown later. TV-mode fullscreen is applied in Rust.
  onMount(() => {
    library.listenForScans();
    session.listen();
    // Controller actions from the Rust gamepad thread go through the same
    // dispatcher as the keyboard.
    void events.onNav((a) => dispatch(a as Action));
    library.reload().catch((e) => console.error("library load failed", e));
  });

  return (
    <main class="app">
      <Switch>
        {/* While a game runs the library is unmounted entirely: no grid, no work. */}
        <Match when={session.running()}>{(g) => <NowPlaying game={g()} />}</Match>
        <Match when={view() === "library"}>
          <LibraryView onOpenSettings={() => setView("settings")} />
        </Match>
        <Match when={view() === "settings"}>
          <SettingsView onClose={() => setView("library")} />
        </Match>
      </Switch>
    </main>
  );
}
