import { createEffect, createSignal, lazy, Match, onCleanup, onMount, Show, Switch } from "solid-js";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, events } from "./api";
import { library } from "./state/library";
import { session } from "./state/session";
import { dispatchFromGamepad, installKeyboard, pushHandler, setActionTap, type Action } from "./input/actions";
import { LibraryView } from "./views/Library";
import { TvMode } from "./views/TvMode";
// Loaded the first time Settings opens, so starting KB doesn't parse it.
const SettingsView = lazy(() => import("./views/Settings").then((m) => ({ default: m.SettingsView })));
import { NowPlaying } from "./views/NowPlaying";
import { OskHost } from "./components/OnScreenKeyboard";
import { ScanToast } from "./components/ScanToast";
import { UpdateDialog } from "./components/UpdateDialog";
import { updates } from "./state/updates";
import { WhatsNew } from "./components/WhatsNew";
import { whatsNew } from "./state/whatsNew";
import { collections } from "./state/collections";
import { fx } from "./state/fx";
import { Intros } from "./fx/Intro";
import { LaunchHost } from "./fx/LaunchFx";
import { sfx, warmUp } from "./fx/audio";
import "./styles/app.css";
import "./styles/fx.css";

type View = "library" | "settings";

export default function App() {
  const [view, setView] = createSignal<View>("library");
  // TV Mode replaces the desktop library (Settings opens over either).
  const [tv, setTv] = createSignal(false);
  // Animation level and sounds follow the mode KB is in.
  createEffect(() => fx.setTv(tv()));
  // Menu sounds: a move, a choice or going back that a screen used.
  let lastSound = 0;
  setActionTap((a) => {
    const role = a === "select" ? "select" : a === "back" ? "back" : ["up", "down", "left", "right", "prevPlatform", "nextPlatform"].includes(a) ? "move" : null;
    const now = performance.now();
    // Held directions repeat fast; one tick per 45 ms is plenty.
    if (!role || (role === "move" && now - lastSound < 45)) return;
    lastSound = now;
    sfx(role);
  });
  createEffect(() => fx.loaded() && warmUp());
  const win = getCurrentWindow();
  // A one-time message (the library moving to KaneegaBell's folder, the TV
  // display not being connected).
  const [notice, setNotice] = createSignal<string | null>(null);
  const showNotice = (text: string, ms = 30_000) => {
    setNotice(text);
    setTimeout(() => setNotice((n) => (n === text ? null : n)), ms);
  };
  // TV Mode goes fullscreen, on the chosen TV display if it's connected;
  // leaving it puts the window back where it was (both done in Rust).
  async function setTvMode(on: boolean) {
    if (on === tv()) return;
    setTv(on);
    await syncTvWindow(on);
  }
  async function syncTvWindow(on: boolean) {
    try {
      if (!on) return await api.tvLeave();
      const r = await api.tvEnter();
      if (r.displayMissing) showNotice("The TV Mode display isn't connected or is off, so TV Mode opened here.", 10_000);
    } catch {
      // Window controls unavailable (browser dev mock): the view still switches.
    }
  }
  void api
    .getSetting<boolean>("startInTv")
    .then((v) => {
      if (v !== true) return;
      setTv(true);
      void syncTvWindow(true); // already placed at startup; reports a missing display
    })
    .catch(() => {});
  void api
    .getStartupNotice()
    .then((n) => n && showNotice(n))
    .catch(() => {});
  // A launch whose emulator closed right away: say why, once.
  createEffect(() => {
    const f = session.failure();
    if (f) {
      showNotice(f, 60_000);
      session.clearFailure();
    }
  });

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
    collections.listen();
    updates.listen();
    session.listen();
    // Controller actions from the Rust gamepad thread go through the same
    // dispatcher as the keyboard.
    void events.onNav((a) => dispatchFromGamepad(a as Action));
    // The update access code is missing (said once) or stopped working.
    void events.onUpdateAccess((message) => showNotice(message, 20_000));
    library
      .reload()
      // Two frames later the grid has been laid out and painted: the app is interactive.
      .then(() => requestAnimationFrame(() => requestAnimationFrame(() => void api.perfMark("interactive").catch(() => {}))))
      .catch((e) => console.error("library load failed", e));
    // Just updated: show what changed (once), after the first paint.
    setTimeout(() => void whatsNew.showIfUpdated().catch(() => {}), 1500);
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
      <UpdateDialog />
      <WhatsNew />
      <OskHost />
      {/* Settings shows the full report itself. */}
      <ScanToast hidden={view() === "settings"} />
      <LaunchHost />
      <Intros />
    </main>
  );
}
