import { createSignal, Match, onCleanup, onMount, Show, Switch } from "solid-js";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, events } from "./api";
import { library } from "./state/library";
import { session } from "./state/session";
import { dispatchFromGamepad, installKeyboard, pushHandler, type Action } from "./input/actions";
import { LibraryView } from "./views/Library";
import { TvMode } from "./views/TvMode";
import { SettingsView } from "./views/Settings";
import { NowPlaying } from "./views/NowPlaying";
import { OskHost } from "./components/OnScreenKeyboard";
import { ScanToast } from "./components/ScanToast";
import "./styles/app.css";

type View = "library" | "settings";

export default function App() {
  const [view, setView] = createSignal<View>("library");
  // TV Mode replaces the desktop library (Settings opens over either).
  const [tv, setTv] = createSignal(false);
  const win = getCurrentWindow();
  // TV Mode goes fullscreen; leaving it restores the window it found.
  let tvMadeFullscreen = false;
  async function setTvMode(on: boolean) {
    if (on === tv()) return;
    setTv(on);
    try {
      if (on && !(await win.isFullscreen())) {
        await win.setFullscreen(true);
        tvMadeFullscreen = true;
      } else if (!on && tvMadeFullscreen) {
        await win.setFullscreen(false);
        tvMadeFullscreen = false;
      }
    } catch {
      // Window controls unavailable (browser dev mock): the view still switches.
    }
  }
  void api
    .getSetting<boolean>("startInTv")
    .then((v) => v === true && setTv(true))
    .catch(() => {});
  // A one-time message from startup (the library moving to KaneegaBell's folder).
  const [notice, setNotice] = createSignal<string | null>(null);
  void api
    .getStartupNotice()
    .then((n) => {
      if (!n) return;
      setNotice(n);
      setTimeout(() => setNotice(null), 30_000);
    })
    .catch(() => {});

  // L3 / F10 anywhere (views handle it first when they want to); B dismisses the notice.
  onCleanup(
    pushHandler((a) => {
      if (a === "back" && notice()) {
        setNotice(null);
        return true;
      }
      if (a !== "toggleTv") return false;
      if (view() === "settings") setView("library");
      void setTvMode(!tv());
      return true;
    }),
  );

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
    void events.onNav((a) => dispatchFromGamepad(a as Action));
    library
      .reload()
      // Two frames later the grid has been laid out and painted: the app is interactive.
      .then(() => requestAnimationFrame(() => requestAnimationFrame(() => void api.perfMark("interactive").catch(() => {}))))
      .catch((e) => console.error("library load failed", e));
  });

  return (
    <main class="app">
      <Switch>
        {/* While a game runs the library is unmounted entirely: no grid, no work. */}
        <Match when={session.running()} keyed>
          {(g) => <NowPlaying game={g} />}
        </Match>
        <Match when={view() === "library" && tv()}>
          <TvMode onExit={() => void setTvMode(false)} onOpenSettings={() => setView("settings")} />
        </Match>
        <Match when={view() === "library"}>
          <LibraryView onOpenSettings={() => setView("settings")} onTvMode={() => void setTvMode(true)} />
        </Match>
        <Match when={view() === "settings"}>
          <SettingsView onClose={() => setView("library")} />
        </Match>
      </Switch>
      <Show when={notice()}>
        <div class="app-notice" role="status">
          <span>{notice()}</span>
          <button class="btn small" onClick={() => setNotice(null)}>
            OK
          </button>
        </div>
      </Show>
      <OskHost />
      {/* Settings shows the full report itself. */}
      <ScanToast hidden={view() === "settings"} />
    </main>
  );
}
