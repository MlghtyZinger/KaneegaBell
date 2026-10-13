import { createEffect, createMemo, createSignal, For, on, onCleanup, Show } from "solid-js";
import { library, type PlatformFilter } from "../state/library";
import { osk } from "../state/osk";
import { pushHandler, type Action } from "../input/actions";
import { VirtualGrid } from "../components/VirtualGrid";
import { GameCard } from "../components/GameCard";
import { Hint } from "../components/Hints";
import { GameInfo } from "./GameInfo";
import { api, type GameRow } from "../api";

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

  // Card shapes: each card follows its own cover (GBA square, SNES landscape,
  // PSP tall...). Cards without a cover take their platform's usual shape.
  const DEFAULT_ASPECT = 1.4;
  const platformAspect = createMemo(
    () => new Map(library.platforms().map((p) => [p.id, p.artAspect ?? DEFAULT_ASPECT])),
  );
  const itemAspect = (g: GameRow) =>
    (g.art > 0 ? g.artAspect : null) ?? platformAspect().get(g.platformId) ?? DEFAULT_ASPECT;
  // A platform tab whose covers are wide gets wider cards (keeping each
  // cover's area roughly constant); mixed views use the portrait default.
  const tabAspect = createMemo(() => {
    if (library.query()) return DEFAULT_ASPECT;
    return platformAspect().get(library.platform()) ?? DEFAULT_ASPECT;
  });
  const minCardWidth = createMemo(() => Math.round(library.cardSize() * Math.sqrt(DEFAULT_ASPECT / tabAspect())));

  // Ctrl + mouse wheel zooms the cards (plain wheel scrolls).
  const onWheel = (e: WheelEvent) => {
    if (!e.ctrlKey || e.deltaY === 0) return;
    e.preventDefault();
    library.zoom(e.deltaY < 0 ? 1 : -1);
  };
  const showPlatformBadge = () => library.query() !== "" || ["all", "favorites", "recent"].includes(library.platform());

  // New tab or new search -> start at the top.
  createEffect(on([library.platform, library.query], () => setFocused(0), { defer: true }));
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

  function openSearch() {
    const before = library.query();
    osk.open({
      title: "Search all games",
      value: before,
      onInput: (v) => library.setQuery(v),
      status: () => {
        const n = library.visibleGames().length;
        return library.query().trim() ? `${n} match${n === 1 ? "" : "es"}` : "Type to search every platform";
      },
      onDone: (v) => library.setQuery(v.trim()),
      onCancel: () => library.setQuery(before),
    });
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
      case "secondary": openSearch(); return true;
      case "select": {
        const g = games()[focused()];
        if (g) setInfoId(g.id);
        return true;
      }
      case "back":
        if (library.query()) {
          library.setQuery("");
          return true;
        }
        return false;
      case "menu": props.onOpenSettings(); return true;
      case "zoomIn": library.zoom(1); return true;
      case "zoomOut": library.zoom(-1); return true;
      default: return false;
    }
  };
  onCleanup(pushHandler(onAction));

  const tabLabel = (id: PlatformFilter) => {
    if (id === "all") return "All";
    if (id === "favorites") return "★ Favorites";
    if (id === "recent") return "Recent";
    return library.platforms().find((x) => x.id === id)?.shortName ?? id;
  };
  const tabCount = (id: PlatformFilter) => {
    if (id === "all") return library.games().length;
    if (id === "favorites") return library.favorites().length;
    if (id === "recent") return library.recent().length;
    return library.platforms().find((x) => x.id === id)?.count ?? 0;
  };

  return (
    <div class="library">
      <header class="topbar">
        <nav class="platform-tabs" aria-label="Platforms">
          <Show
            when={!library.query()}
            fallback={
              <button class="tab active" onClick={() => library.setQuery("")} title="Clear search">
                Search: “{library.query()}” <span class="tab-count">✕</span>
              </button>
            }
          >
            <For each={library.platformTabs()}>
              {(id) => (
                <button
                  class="tab"
                  classList={{ active: library.platform() === id }}
                  onClick={() => library.setPlatform(id)}
                >
                  {tabLabel(id)}
                  <span class="tab-count">{tabCount(id)}</span>
                </button>
              )}
            </For>
          </Show>
        </nav>
        <div class="topbar-right">
          <Show when={library.scanProgress()}>
            {(p) => <span class="scan-pill">Scanning {p().total > 0 ? `${p().done + 1}/${p().total}` : "…"}</span>}
          </Show>
          <Show when={!library.scanProgress() && library.artProgress()}>
            {(p) => (
              <span class="scan-pill">
                Covers {p().done}/{p().total}
              </span>
            )}
          </Show>
          <button class="btn ghost" onClick={openSearch}>
            Search
          </button>
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
              <Show
                when={library.games().length > 0}
                fallback={
                  <>
                    <h2>No games yet</h2>
                    <p>Add your ROM folders and emulator paths in Settings, then run a scan.</p>
                    <button class="btn primary" onClick={props.onOpenSettings}>
                      Open Settings
                    </button>
                  </>
                }
              >
                <h2>No matches</h2>
                <p>Nothing matches “{library.query()}”.</p>
                <div class="hints-row">
                  <Hint action="secondary" label="Search again" />
                  <Hint action="back" label="Clear search" />
                </div>
              </Show>
            </Show>
          </div>
        }
      >
        <div class="grid-wrap" onWheel={onWheel}>
          <VirtualGrid
            items={games()}
            focused={focused()}
            onFocus={setFocused}
            onActivate={(i) => setInfoId(games()[i].id)}
            onColumns={setCols}
            minCardWidth={minCardWidth()}
            itemAspect={itemAspect}
            gap={18}
            renderItem={(g, isFocused) => (
              <GameCard
                game={g}
                focused={isFocused}
                platformLabel={showPlatformBadge() ? shortNames().get(g().platformId) : undefined}
              />
            )}
          />
        </div>
        <footer class="hints">
          <Hint action="select" label="Details" />
          <Hint action="secondary" label="Search" />
          <Hint action="favorite" label="Favorite" />
          <Hint actions={["prevPlatform", "nextPlatform"]} label="Platform" />
          <Hint actions={["prevLetter", "nextLetter"]} label="Letter" />
          <Hint action="menu" label="Settings" />
          <Hint actions={["zoomOut", "zoomIn"]} label="Card size" />
          <input
            class="zoom-slider"
            type="range"
            aria-label="Card size"
            min={library.CARD_MIN}
            max={library.CARD_MAX}
            step={5}
            value={library.cardSize()}
            onInput={(e) => library.setCardSize(Number(e.currentTarget.value))}
            // Give the arrow keys back to the grid once the drag ends.
            onPointerUp={(e) => e.currentTarget.blur()}
            tabIndex={-1}
          />
          <span class="hint-count">
            {focused() + 1} / {games().length}
          </span>
        </footer>
      </Show>

      <Show when={infoId()} keyed>
        {(id) => <GameInfo id={id} onClose={() => setInfoId(null)} />}
      </Show>
    </div>
  );
}
