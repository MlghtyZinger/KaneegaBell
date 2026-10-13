import { createEffect, createMemo, createSignal, For, on, onCleanup, onMount, Show } from "solid-js";
import { library, type PlatformFilter } from "../state/library";
import { osk } from "../state/osk";
import { pushHandler, type Action } from "../input/actions";
import { VirtualGrid, type NavDir, type Navigator } from "../components/VirtualGrid";
import { GameCard } from "../components/GameCard";
import { Hint } from "../components/Hints";
import { Picker } from "../components/Picker";
import { GameInfo } from "./GameInfo";
import { api, type GameRow } from "../api";

/** What the background worker is doing (one job at a time). */
function JobPill() {
  const text = () => {
    const scan = library.scanProgress();
    if (scan) return `Scanning ${scan.total > 0 ? `${scan.done + 1}/${scan.total}` : "…"}`;
    const jobs: [string, { done: number; total: number } | null][] = [
      ["Covers", library.artProgress()],
      ["Game info", library.metaProgress()],
      ["Artwork", library.sgdbProgress()],
      ["Achievements", library.raProgress()],
    ];
    const active = jobs.find(([, p]) => p);
    return active ? `${active[0]} ${active[1]!.done}/${active[1]!.total}` : null;
  };
  return <Show when={text()}>{(t) => <span class="scan-pill">{t()}</span>}</Show>;
}

function initial(title: string): string {
  const c = title.replace(/^(the|a|an)\s+/i, "").charAt(0).toUpperCase();
  return c >= "A" && c <= "Z" ? c : "#";
}

export function LibraryView(props: { onOpenSettings: () => void }) {
  const { focused, setFocused } = library;
  const [infoId, setInfoId] = createSignal<number | null>(null);
  const [genreOpen, setGenreOpen] = createSignal(false);
  const genreOptions = createMemo(() => [
    { value: "", label: "All genres" },
    ...library
      .genres()
      .filter((g) => library.genreCounts().has(g.id) || g.id === library.genre())
      .map((g) => ({ value: String(g.id), label: `${g.name} (${library.genreCounts().get(g.id) ?? 0})` })),
  ]);
  const genreName = () => library.genres().find((g) => g.id === library.genre())?.name;
  // The grid knows where cards are (rows, sections): D-pad moves ask it.
  let navigate: Navigator | undefined;

  const games = library.visibleGames;
  const shortNames = createMemo(() => new Map(library.platforms().map((p) => [p.id, p.shortName])));

  // Views mixing platforms (All, Favorites, search) show one section per
  // platform, so each row holds covers of one shape. Recent stays a single
  // list in play order.
  const mixed = () => library.query() !== "" || ["all", "favorites", "recent"].includes(library.platform());
  const sectioned = () => library.grouped() && mixed() && library.platform() !== "recent";
  const sectionCounts = createMemo(() => {
    const counts = new Map<string, number>();
    for (const g of games()) counts.set(g.platformId, (counts.get(g.platformId) ?? 0) + 1);
    return counts;
  });
  const platformNames = createMemo(() => new Map(library.platforms().map((p) => [p.id, p.name])));
  const sectionLabel = (id: string) => `${platformNames().get(id) ?? id.toUpperCase()} · ${sectionCounts().get(id) ?? 0}`;

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
  // In sections the header names the platform; elsewhere mixed views badge each card.
  const showPlatformBadge = () => mixed() && !sectioned();

  // New tab, search or genre -> start at the top.
  createEffect(on([library.platform, library.query, library.genre], () => setFocused(0), { defer: true }));
  // Clamp when the list shrinks (rescan).
  createEffect(() => {
    const n = games().length;
    if (focused() >= n) setFocused(Math.max(0, n - 1));
  });

  function move(dir: NavDir) {
    if (games().length === 0 || !navigate) return;
    setFocused(navigate(focused(), dir));
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
      case "left":
      case "right":
      case "up":
      case "down":
        move(a);
        return true;
      case "toggleGroups": library.setGrouped(!library.grouped()); return true;
      case "filter":
        if (library.genres().length > 0) setGenreOpen(true);
        return true;
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
        if (library.genre() !== null) {
          library.setGenre(null);
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
  // The tab row scrolls sideways when it doesn't fit: keep the active tab in
  // view, and fade the edge(s) that have more tabs past them.
  let tabsEl!: HTMLElement;
  const [tabFade, setTabFade] = createSignal({ left: false, right: false });
  const updateTabFade = () => {
    const el = tabsEl;
    setTabFade({ left: el.scrollLeft > 2, right: el.scrollLeft + el.clientWidth < el.scrollWidth - 2 });
  };
  createEffect(
    on([library.platform, library.platformTabs, library.query], () =>
      queueMicrotask(() => {
        tabsEl?.querySelector(".tab.active")?.scrollIntoView({ block: "nearest", inline: "nearest" });
        updateTabFade();
      }),
    ),
  );
  onMount(() => {
    const ro = new ResizeObserver(updateTabFade);
    ro.observe(tabsEl);
    onCleanup(() => ro.disconnect());
  });

  const tabCount = (id: PlatformFilter) => {
    if (id === "all") return library.games().length;
    if (id === "favorites") return library.favorites().length;
    if (id === "recent") return library.recent().length;
    return library.platforms().find((x) => x.id === id)?.count ?? 0;
  };

  return (
    <div class="library">
      <header class="topbar">
        <nav
          class="platform-tabs"
          classList={{ "fade-left": tabFade().left, "fade-right": tabFade().right }}
          aria-label="Platforms"
          ref={tabsEl}
          onScroll={updateTabFade}
        >
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
          {/* With games, progress sits in the footer, leaving room for the tabs. */}
          <Show when={games().length === 0}>
            <JobPill />
          </Show>
          <Show when={library.genres().length > 0}>
            <Picker
              class={`genre-picker${library.genre() !== null ? " active" : ""}`}
              title="Show genre"
              value={library.genre() === null ? "" : String(library.genre())}
              options={genreOptions()}
              onChange={(v) => library.setGenre(v === "" ? null : Number(v))}
              open={genreOpen()}
              onOpenChange={setGenreOpen}
              label={genreName() ? `Genre: ${genreName()}` : "Genre"}
            />
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
                <p>
                  {library.query() ? `Nothing matches “${library.query()}”` : "No games here"}
                  {genreName() ? ` in ${genreName()}` : ""}.
                </p>
                <div class="hints-row">
                  <Hint action="secondary" label="Search again" />
                  <Hint action="back" label={library.query() ? "Clear search" : "All genres"} />
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
            onNavigator={(n) => (navigate = n)}
            minCardWidth={minCardWidth()}
            itemAspect={itemAspect}
            groupOf={sectioned() ? (g) => g.platformId : undefined}
            groupLabel={sectionLabel}
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
          <Show when={library.genres().length > 0}>
            <Hint action="filter" label="Genre" />
          </Show>
          <Hint actions={["prevPlatform", "nextPlatform"]} label="Platform" />
          <Hint actions={["prevLetter", "nextLetter"]} label="Letter" />
          <Hint action="menu" label="Settings" />
          <Hint actions={["zoomOut", "zoomIn"]} label="Card size" />
          <Show when={mixed() && library.platform() !== "recent"}>
            <Hint action="toggleGroups" label={library.grouped() ? "One list" : "Sections"} />
          </Show>
          <span class="footer-jobs">
            <JobPill />
          </span>
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
