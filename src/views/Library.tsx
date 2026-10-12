import { createEffect, createMemo, createSignal, For, on, onCleanup, Show } from "solid-js";
import { library } from "../state/library";
import { pushHandler, type Action } from "../input/actions";
import { VirtualGrid } from "../components/VirtualGrid";
import { GameCard } from "../components/GameCard";
import { GameInfo } from "./GameInfo";
import { api } from "../api";

function initial(title: string): string {
  const c = title.replace(/^(the|a|an)\s+/i, "").charAt(0).toUpperCase();
  return c >= "A" && c <= "Z" ? c : "#";
}

export function LibraryView(props: { onOpenSettings: () => void }) {
  const { focused, setFocused } = library;
  const [cols, setCols] = createSignal(1);
  const [infoId, setInfoId] = createSignal<number | null>(null);

  const games = library.visibleGames;
  const shortNames = createMemo(() => new Map(library.platforms().map((p) => [p.id, p.shortName])));

  // New platform -> start at the top.
  createEffect(on(library.platform, () => setFocused(0), { defer: true }));
  // Clamp when the list shrinks (rescan).
  createEffect(() => {
    const n = games().length;
    if (focused() >= n) setFocused(Math.max(0, n - 1));
  });

  function move(delta: number) {
    const n = games().length;
    if (n === 0) return;
    setFocused((i) => Math.min(n - 1, Math.max(0, i + delta)));
  }

  function jumpLetter(dir: 1 | -1) {
    const list = games();
    if (list.length === 0) return;
    const cur = initial(list[focused()].title);
    if (dir > 0) {
      const i = list.findIndex((g, k) => k > focused() && initial(g.title) !== cur);
      if (i >= 0) setFocused(i);
    } else {
      // Go to the start of the current letter, or of the previous letter if already there.
      let i = focused();
      while (i > 0 && initial(list[i - 1].title) === cur) i--;
      if (i === focused() && i > 0) {
        const prev = initial(list[i - 1].title);
        i--;
        while (i > 0 && initial(list[i - 1].title) === prev) i--;
      }
      setFocused(i);
    }
  }

  async function toggleFavorite() {
    const g = games()[focused()];
    if (!g) return;
    await api.setFavorite(g.id, !g.favorite);
    await library.reload();
  }

  const onAction = (a: Action): boolean => {
    switch (a) {
      case "left": move(-1); return true;
      case "right": move(1); return true;
      case "up": move(-cols()); return true;
      case "down": move(cols()); return true;
      case "prevPlatform": library.cyclePlatform(-1); return true;
      case "nextPlatform": library.cyclePlatform(1); return true;
      case "prevLetter": jumpLetter(-1); return true;
      case "nextLetter": jumpLetter(1); return true;
      case "favorite": void toggleFavorite(); return true;
      case "select": {
        const g = games()[focused()];
        if (g) setInfoId(g.id);
        return true;
      }
      case "menu": props.onOpenSettings(); return true;
      default: return false;
    }
  };
  onCleanup(pushHandler(onAction));

  return (
    <div class="library">
      <header class="topbar">
        <nav class="platform-tabs" aria-label="Platforms">
          <For each={library.platformTabs()}>
            {(id) => {
              const p = () => library.platforms().find((x) => x.id === id);
              const count = () => (id === "all" ? library.games().length : p()?.count ?? 0);
              return (
                <button
                  class="tab"
                  classList={{ active: library.platform() === id }}
                  onClick={() => library.setPlatform(id)}
                >
                  {id === "all" ? "All" : p()?.shortName}
                  <span class="tab-count">{count()}</span>
                </button>
              );
            }}
          </For>
        </nav>
        <div class="topbar-right">
          <Show when={library.scanProgress()}>
            {(p) => (
              <span class="scan-pill">
                Scanning {p().total > 0 ? `${p().done + 1}/${p().total}` : "…"}
              </span>
            )}
          </Show>
          <button class="btn ghost" onClick={props.onOpenSettings}>
            Settings
          </button>
        </div>
      </header>

      <Show
        when={games().length > 0}
        fallback={
          <div class="empty">
            <Show when={library.loaded()} fallback={<p>Loading…</p>}>
              <h2>No games yet</h2>
              <p>Add your ROM folders and emulator paths in Settings, then run a scan.</p>
              <button class="btn primary" onClick={props.onOpenSettings}>
                Open Settings
              </button>
            </Show>
          </div>
        }
      >
        <div class="grid-wrap">
          <VirtualGrid
            items={games()}
            focused={focused()}
            onFocus={setFocused}
            onActivate={(i) => setInfoId(games()[i].id)}
            onColumns={setCols}
            minCardWidth={210}
            aspect={1.4}
            gap={18}
            renderItem={(g, isFocused) => (
              <GameCard
                game={g}
                focused={isFocused}
                platformLabel={library.platform() === "all" ? shortNames().get(g().platformId) : undefined}
              />
            )}
          />
        </div>
        <footer class="hints">
          <span><kbd>Enter</kbd> Details</span>
          <span><kbd>Q</kbd>/<kbd>E</kbd> Platform</span>
          <span><kbd>Home</kbd>/<kbd>End</kbd> Letter</span>
          <span><kbd>F</kbd> Favorite</span>
          <span class="hint-count">
            {focused() + 1} / {games().length}
          </span>
        </footer>
      </Show>

      <Show when={infoId()}>{(id) => <GameInfo id={id()} onClose={() => setInfoId(null)} />}</Show>
    </div>
  );
}
