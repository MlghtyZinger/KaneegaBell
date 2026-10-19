import { createEffect, createMemo, createSignal, For, on, onCleanup, Show } from "solid-js";
import { collectionKeyOf, library, type PlatformFilter } from "../state/library";
import { collections } from "../state/collections";
import {
  CollectionBanner,
  CollectionEditor,
  CollectionTile,
  NewCollectionTile,
  bannerActions,
  newCollection,
  openCollection,
  renameCollection,
  saveSmartCollection,
} from "../components/Collections";
import type { Collection } from "../api";
import appIcon from "../assets/app-icon.png";
import { Icon, PlatformIcon, type IconName } from "../components/Icons";
import { osk } from "../state/osk";
import { countActive } from "../state/smartFilter";
import { pushHandler, type Action } from "../input/actions";
import { VirtualGrid, type NavDir, type Navigator } from "../components/VirtualGrid";
import { GameCard } from "../components/GameCard";
import { GameListRow } from "../components/GameListRow";
import { Hint } from "../components/Hints";
import { FilterPanel } from "../components/FilterPanel";
import { GameInfo } from "./GameInfo";
import { GameSidebar } from "../components/GameSidebar";
import { api, errorText, type GameRow } from "../api";

/** What the background worker is doing (one job at a time). */
function JobPill() {
  const text = createMemo(() => {
    const scan = library.scanProgress();
    if (scan) return `Scanning ${scan.total > 0 ? `${scan.done + 1}/${scan.total}` : "…"}`;
    const lb = library.lbProgress();
    if (lb) {
      const mb = (n: number) => Math.round(n / 1_000_000);
      return {
        checking: "LaunchBox: checking for updates",
        downloading: lb.total > 0 ? `LaunchBox data ${mb(lb.done)}/${mb(lb.total)} MB` : "LaunchBox data…",
        matching: "LaunchBox: matching games",
        images: `LaunchBox pictures ${lb.done}/${lb.total}`,
      }[lb.phase];
    }
    const wd = library.wdProgress();
    if (wd?.platform) {
      const step = wd.step === "games" ? "downloading its game list" : "looking up series";
      return `Series (Wikidata): ${wd.platform}, ${step} (${wd.done + 1}/${wd.total})`;
    }
    const jobs: [string, { done: number; total: number } | null][] = [
      ["Series (Wikidata)", wd],
      ["Covers", library.artProgress()],
      ["Game info", library.metaProgress()],
      ["Artwork", library.sgdbProgress()],
      ["Achievements", library.raProgress()],
    ];
    const active = jobs.find(([, p]) => p);
    return active ? `${active[0]} ${active[1]!.done}/${active[1]!.total}` : null;
  });
  // A step that takes a while (a big download, a slow server) shows its
  // seconds, so a slow step doesn't look stuck. Ticks only while a job runs.
  const [now, setNow] = createSignal(Date.now());
  let changedAt = Date.now();
  createEffect(on(text, () => (changedAt = Date.now())));
  createEffect(
    on(
      () => !!text(),
      (busy) => {
        if (!busy) return;
        const id = setInterval(() => setNow(Date.now()), 1000);
        onCleanup(() => clearInterval(id));
      },
    ),
  );
  const shown = () => {
    const t = text();
    const secs = Math.floor((now() - changedAt) / 1000);
    return t && secs >= 3 ? `${t} · ${secs} s` : t;
  };
  return (
    <Show when={shown()}>
      {(t) => (
        <span class="scan-pill" title={t()}>
          {t()}
        </span>
      )}
    </Show>
  );
}

function initial(title: string): string {
  const c = title.replace(/^(the|a|an)\s+/i, "").charAt(0).toUpperCase();
  return c >= "A" && c <= "Z" ? c : "#";
}

export function LibraryView(props: { onOpenSettings: () => void; onTvMode: () => void }) {
  const { focused, setFocused } = library;
  // The full details screen (△ / F / right-click) and the sidebar (A / Enter / double-click).
  const [infoId, setInfoId] = createSignal<number | null>(null);
  const [sideOpen, setSideOpen] = createSignal(false);
  const focusedGame = () => (overview() ? undefined : games()[focused()]);
  // The sidebar shows the focused game; a click on another cover moves it there.
  const sideId = () => (sideOpen() ? (focusedGame()?.id ?? null) : null);
  const openSide = (i: number) => {
    setFocused(i);
    setSideOpen(true);
  };
  const openDetails = (i: number) => {
    const g = games()[i];
    if (g) setInfoId(g.id);
  };
  const [filterOpen, setFilterOpen] = createSignal(false);
  const filterLabel = () => (library.activeFilter() ? `Filter (${countActive(library.filter())})` : "Filter");
  // The grid knows where cards are (rows, sections): D-pad moves ask it.
  let navigate: Navigator | undefined;

  const games = library.visibleGames;

  // Collections: the overview (tiles) and one open collection ("col:<key>").
  type Tile = { kind: "new" } | { kind: "col"; c: Collection } | { kind: "hidden" };
  const overview = () => library.platform() === "collections" && !library.query();
  const openKey = () => (library.query() ? null : collectionKeyOf(library.platform()));
  const openCol = () => {
    const k = openKey();
    return k ? collections.byKey().get(k) : undefined;
  };
  const [collectionError, setCollectionError] = createSignal<string | null>(null);
  async function removeCollectionMember(c: Collection, game: GameRow) {
    if (c.kind === "smart") return;
    setCollectionError(null);
    try {
      await api.setInCollection(c.key, game.id, false);
      await collections.reload();
    } catch (e) {
      setCollectionError(errorText(e));
    }
  }
  const tiles = createMemo<Tile[]>(() => [
    { kind: "new" },
    ...collections.visible().map((c) => ({ kind: "col" as const, c })),
    ...(collections.hiddenCount() > 0 ? [{ kind: "hidden" as const }] : []),
  ]);
  const [editKey, setEditKey] = createSignal<string | null>(null);
  const editing = () => {
    const k = editKey();
    return k ? collections.byKey().get(k) : undefined;
  };
  // Coming back from a collection lands on its tile.
  let lastTile = 0;
  function activateTile(i: number) {
    const t = tiles()[i];
    if (!t) return;
    if (t.kind === "new") newCollection(null, (key) => openCollection(key));
    else if (t.kind === "hidden") collections.setShowHidden(!collections.showHidden());
    else {
      lastTile = i;
      openCollection(t.c.key);
    }
  }
  const editTile = (i: number) => {
    const t = tiles()[i];
    if (t?.kind === "col") setEditKey(t.c.key);
  };
  const shortNames = createMemo(() => new Map(library.platforms().map((p) => [p.id, p.shortName])));

  // Views mixing platforms (All, Favorites, search) show one section per
  // platform, so each row holds covers of one shape. Recent stays a single
  // list in play order.
  const mixed = () => library.query() !== "" || ["all", "favorites", "recent"].includes(library.platform()) || !!openKey();
  // A collection stays one list in its own order (like Recent).
  const sectioned = () => mixed() && library.platform() !== "recent" && !openKey();
  /** List rows are this tall (px). */
  const LIST_ROW_H = 40;
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

  // New tab, search or genre -> start at the top (back in Collections: on the tile you left).
  createEffect(
    on([library.platform, library.query, library.filterKey], () => {
      setSideOpen(false);
      setFocused(overview() ? Math.min(lastTile, tiles().length - 1) : 0);
    }, {
      defer: true,
    }),
  );
  // Clamp when the list shrinks (rescan).
  createEffect(() => {
    const n = overview() ? tiles().length : games().length;
    if (focused() >= n) setFocused(Math.max(0, n - 1));
  });

  // The list view's panel: drag its left edge (double-click resets), or + / − / right stick.
  let splitEl: HTMLDivElement | undefined;
  const panelNow = () => splitEl?.querySelector<HTMLElement>(".game-side")?.offsetWidth ?? 0;
  function resizePanel(delta: number) {
    library.setPanelWidth(panelNow() + delta);
  }
  function dragPanel(e: PointerEvent) {
    const handle = e.currentTarget as HTMLElement;
    handle.setPointerCapture(e.pointerId);
    const right = splitEl!.getBoundingClientRect().right;
    const move = (ev: PointerEvent) => library.setPanelWidth(right - ev.clientX, false);
    const done = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", done);
      handle.removeEventListener("pointercancel", done);
      library.setPanelWidth(library.panelWidth());
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", done);
    handle.addEventListener("pointercancel", done);
  }

  function move(dir: NavDir) {
    const n = overview() ? tiles().length : games().length;
    // List view: right goes to the details panel (like A).
    if (dir === "right" && library.listView() && !overview()) {
      if (games()[focused()]) setSideOpen(true);
      return;
    }
    // Left from the first column (or an empty view) moves into the rail.
    if (dir === "left" && (n === 0 || !navigate || navigate.atRowStart(focused()))) {
      openRail();
      return;
    }
    // Up from the top row (or an empty view): an open collection's banner
    // buttons (Rename for your own, else Edit), otherwise the top bar.
    if (dir === "up" && (n === 0 || !navigate || navigate(focused(), "up") === focused())) {
      const col = openCol();
      if (col) {
        const acts = bannerActions(col);
        setBannerFocus(acts.includes("rename" as never) ? "rename" : "edit");
      } else setTopFocus("search");
      return;
    }
    if (n === 0 || !navigate) return;
    setFocused(navigate(focused(), dir));
  }

  // Controller focus on the top bar (up from the top row), or null.
  const [topFocus, setTopFocus] = createSignal<string | null>(null);
  const topActions = () => ["search", "filter", "tv", "settings"];
  function topAction(a: Action): boolean {
    const acts = topActions();
    const i = Math.max(0, acts.indexOf(topFocus()!));
    switch (a) {
      case "left": setTopFocus(acts[Math.max(0, i - 1)]); return true;
      case "right": setTopFocus(acts[Math.min(acts.length - 1, i + 1)]); return true;
      case "down":
      case "back": setTopFocus(null); return true;
      case "up": return true;
      case "select": {
        const f = topFocus();
        setTopFocus(null);
        if (f === "search") openSearch();
        else if (f === "filter") setFilterOpen(true);
        else if (f === "tv") props.onTvMode();
        else props.onOpenSettings();
        return true;
      }
      default: return false;
    }
  }

  // Controller focus on an open collection's banner buttons, or null.
  const [bannerFocus, setBannerFocus] = createSignal<string | null>(null);
  createEffect(on(library.platform, () => setBannerFocus(null), { defer: true }));
  function bannerAction(a: Action): boolean {
    const c = openCol();
    if (!c) {
      setBannerFocus(null);
      return false;
    }
    const acts: readonly string[] = bannerActions(c);
    const i = Math.max(0, acts.indexOf(bannerFocus()!));
    switch (a) {
      case "left": setBannerFocus(acts[Math.max(0, i - 1)]); return true;
      case "right": setBannerFocus(acts[Math.min(acts.length - 1, i + 1)]); return true;
      case "down":
      case "back": setBannerFocus(null); return true;
      case "up":
        setBannerFocus(null);
        setTopFocus("search");
        return true;
      case "select": {
        const f = bannerFocus();
        setBannerFocus(null);
        if (f === "back") library.setPlatform("collections");
        else if (f === "rename") renameCollection(c);
        else setEditKey(c.key);
        return true;
      }
      default: return false;
    }
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
    if (railFocus() !== null && railAction(a)) return true;
    if (bannerFocus() !== null && bannerAction(a)) return true;
    if (topFocus() !== null && topAction(a)) return true;
    switch (a) {
      case "left":
      case "right":
      case "up":
      case "down":
        move(a);
        return true;
      case "toggleGroups": library.setListView(!library.listView()); return true;
      case "filter":
        setFilterOpen(true);
        return true;
      case "prevPlatform": library.cyclePlatform(-1); return true;
      case "nextPlatform": library.cyclePlatform(1); return true;
      case "prevLetter": if (!overview()) jumpLetter(-1); return true;
      case "nextLetter": if (!overview()) jumpLetter(1); return true;
      // △ / Y / F: the full details screen (in the Collections overview: edit the collection).
      case "favorite":
        if (overview()) editTile(focused());
        else if (openCol() && openCol()!.kind !== "smart" && games()[focused()]) void removeCollectionMember(openCol()!, games()[focused()]);
        else openDetails(focused());
        return true;
      case "secondary": openSearch(); return true;
      case "select": {
        if (overview()) activateTile(focused());
        else if (games()[focused()]) openSide(focused());
        return true;
      }
      case "back":
        if (library.query()) {
          library.setQuery("");
          return true;
        }
        if (library.activeFilter()) {
          library.clearFilter();
          return true;
        }
        if (openKey()) {
          library.setPlatform("collections");
          return true;
        }
        return false;
      case "menu": props.onOpenSettings(); return true;
      // List view: the same keys (+ / − / right stick) widen and narrow the panel.
      case "zoomIn": if (library.listView()) resizePanel(40); else library.zoom(1); return true;
      case "zoomOut": if (library.listView()) resizePanel(-40); else library.zoom(-1); return true;
      default: return false;
    }
  };
  onCleanup(pushHandler(onAction));

  const tabLabel = (id: PlatformFilter) => {
    if (id === "all") return "All games";
    if (id === "favorites") return "Favorites";
    if (id === "recent") return "Recent";
    if (id === "collections") return "Collections";
    return library.platforms().find((x) => x.id === id)?.name ?? id;
  };
  const LIBRARY_ICONS: Record<string, IconName> = { all: "all", favorites: "star", recent: "clock", collections: "stack" };
  const tabIcon = (id: PlatformFilter) =>
    LIBRARY_ICONS[id] ? <Icon name={LIBRARY_ICONS[id]} /> : <PlatformIcon id={id} />;
  const tabActive = (id: PlatformFilter) => library.platform() === id || (id === "collections" && !!collectionKeyOf(library.platform()));
  const tabCount = (id: PlatformFilter) => {
    if (id === "all") return library.games().length;
    if (id === "favorites") return library.favorites().length;
    if (id === "recent") return library.recent().length;
    if (id === "collections") return collections.visible().length;
    return library.platforms().find((x) => x.id === id)?.count ?? 0;
  };
  const firstPlatformTab = () => library.platformTabs().findIndex((t) => library.platforms().some((p) => p.id === t));

  // Where you are, for the top bar (the rail shows only icons most of the time).
  const here = () => {
    // Moving through a closed rail: name the focused tab.
    const rf = railFocus();
    if (rf !== null && railMode() === "closed") {
      const t = library.platformTabs()[rf];
      if (t) return { title: tabLabel(t), count: t === "collections" ? "" : `${tabCount(t)} games` };
      return { title: `Sidebar: ${RAIL_MODES[railMode()].label}`, count: "A to change" };
    }
    const n = games().length;
    const g = `${n} game${n === 1 ? "" : "s"}`;
    if (library.query()) return { title: `Search: “${library.query()}”`, count: g };
    const c = openCol();
    if (c) return { title: c.name, count: g };
    const p = library.platform();
    if (p === "collections") {
      const k = collections.visible().length;
      return { title: "Collections", count: `${k} collection${k === 1 ? "" : "s"}` };
    }
    return { title: tabLabel(p), count: g };
  };

  // The rail on the left: icons; it opens (icon + name + count) on hover or
  // when the controller moves into it (left from the first column). Its mode
  // button (at the bottom) cycles: opens on hover -> always open -> always closed.
  type RailMode = "hover" | "open" | "closed";
  const RAIL_MODES: Record<RailMode, { next: RailMode; label: string; tip: string }> = {
    hover: { next: "open", label: "Opens on hover", tip: "Sidebar opens on hover. Click to keep it open." },
    open: { next: "closed", label: "Always open", tip: "Sidebar always open. Click to keep it closed." },
    closed: { next: "hover", label: "Always closed", tip: "Sidebar always closed (icons only). Click to open it on hover." },
  };
  const [railHover, setRailHover] = createSignal(false);
  const [railFocus, setRailFocus] = createSignal<number | null>(null);
  const [railMode, setRailModeRaw] = createSignal<RailMode>("hover");
  void api
    .getSetting<string>("railMode")
    .then(async (v) => {
      if (v && v in RAIL_MODES) setRailModeRaw(v as RailMode);
      // "Keep open" from the first version of the rail.
      else if ((await api.getSetting<boolean>("railPinned")) === true) setRailModeRaw("open");
    })
    .catch(() => {});
  function setRailMode(m: RailMode) {
    setRailModeRaw(m);
    void api.setSetting("railMode", m).catch(() => {});
  }
  const railPinned = () => railMode() === "open";
  // Always closed: icons only, also while the controller moves through them
  // (the top bar names the focused one).
  const railOpen = () => railMode() !== "closed" && (railPinned() || railHover() || railFocus() !== null);
  const openRail = () => setRailFocus(Math.max(0, library.platformTabs().findIndex(tabActive)));
  const closeRail = () => setRailFocus(null);
  function pickTab(id: PlatformFilter) {
    library.setQuery("");
    library.setPlatform(id);
    closeRail();
    setRailHover(false);
  }
  /** Controller/keyboard while the rail has focus; false = not handled here. */
  function railAction(a: Action): boolean {
    const tabs = library.platformTabs();
    const i = railFocus()!;
    switch (a) {
      case "up": setRailFocus(Math.max(0, i - 1)); return true;
      // One stop past the last tab: the mode button (A cycles the mode).
      case "down": setRailFocus(Math.min(tabs.length, i + 1)); return true;
      case "select":
        if (i >= tabs.length) setRailMode(RAIL_MODES[railMode()].next);
        else pickTab(tabs[i]);
        return true;
      case "back":
      case "right": closeRail(); return true;
      case "left": return true;
      default: return false;
    }
  }
  let railEl: HTMLDivElement | undefined;
  createEffect(() => {
    const i = railFocus();
    if (i !== null) queueMicrotask(() => railEl?.querySelectorAll(".rail-row")[i]?.scrollIntoView({ block: "nearest" }));
  });

  return (
    <div class="library">
      <header class="topbar slim">
        <span class="kb-mark" aria-hidden="true">
          <img src={appIcon} alt="" />
        </span>
        <div class="here">
          <span class="here-title">{here().title}</span>
          <span class="here-count">{here().count}</span>
        </div>
        <div class="topbar-right">
          {/* Background progress lives here: the footer is full of key hints. */}
          <JobPill />
          <button
            class="search-box"
            classList={{ active: !!library.query(), "nav-focus": topFocus() === "search" }}
            onClick={openSearch}
            title="Search every platform"
          >
            <Icon name="search" />
            <span>{library.query() || "Search games"}</span>
          </button>
          <Show when={library.query()}>
            <button class="icon-btn" onClick={() => library.setQuery("")} title="Clear search">
              <Icon name="close" />
            </button>
          </Show>
          <button
            class={`filter-btn compact${library.activeFilter() ? " active" : ""}${topFocus() === "filter" ? " nav-focus" : ""}`}
            onClick={() => setFilterOpen(true)}
            title="Filter games"
          >
            <Icon name="filter" />
            <span>{filterLabel()}</span>
          </button>
          <button class="icon-btn" classList={{ "nav-focus": topFocus() === "tv" }} onClick={props.onTvMode} title="TV Mode (L3 / F10)">
            <Icon name="tv" />
          </button>
          <button class="icon-btn" classList={{ "nav-focus": topFocus() === "settings" }} onClick={props.onOpenSettings} title="Settings">
            <Icon name="gear" />
          </button>
        </div>
      </header>

      <div class="lib-body" classList={{ "rail-pinned": railPinned() }}>
      <nav
        class="rail"
        classList={{ open: railOpen() }}
        aria-label="Library and platforms"
        onMouseEnter={() => setRailHover(true)}
        onMouseLeave={() => setRailHover(false)}
      >
        <div class="rail-list" ref={railEl}>
          <For each={library.platformTabs()}>
            {(id, i) => (
              <>
                <Show when={i() === firstPlatformTab()}>
                  <div class="rail-divider" />
                </Show>
                <button
                  class="rail-row"
                  classList={{ active: tabActive(id), focused: railFocus() === i() }}
                  title={tabLabel(id)}
                  tabIndex={-1}
                  onClick={() => pickTab(id)}
                >
                  <span class="rail-icon">{tabIcon(id)}</span>
                  <span class="rail-name">{tabLabel(id)}</span>
                  <span class="rail-count">{tabCount(id)}</span>
                </button>
              </>
            )}
          </For>
        </div>
        <button
          class="rail-row rail-pin"
          classList={{ active: railMode() !== "hover", focused: railFocus() === library.platformTabs().length }}
          tabIndex={-1}
          title={RAIL_MODES[railMode()].tip}
          onClick={() => setRailMode(RAIL_MODES[railMode()].next)}
        >
          <span class="rail-icon">
            <Icon name={railMode() === "open" ? "pin" : railMode() === "closed" ? "railClosed" : "railHover"} />
          </span>
          <span class="rail-name">{RAIL_MODES[railMode()].label}</span>
        </button>
      </nav>
      <div class="lib-main" classList={{ "focus-away": railFocus() !== null || topFocus() !== null || bannerFocus() !== null || sideOpen() }}>

      <Show
        when={!overview()}
        fallback={
          <>
            <div class="grid-wrap" onWheel={onWheel}>
              <VirtualGrid
                items={tiles()}
                focused={focused()}
                onFocus={setFocused}
                onActivate={activateTile}
                onNavigator={(n) => (navigate = n)}
                minCardWidth={Math.round(library.cardSize() * 1.35)}
                itemAspect={(t) => (t.kind === "col" ? 0.62 : 0.62)}
                gap={18}
                renderItem={(t, isFocused) => (
                  <Show
                    when={t().kind === "col" && (t() as { c: Collection }).c}
                    fallback={
                      t().kind === "new" ? (
                        <NewCollectionTile focused={isFocused} />
                      ) : (
                        <div class="col-tile col-new" classList={{ focused: isFocused() }}>
                          <span>{collections.showHidden() ? "Hide hidden series" : `Show hidden series (${collections.hiddenCount()})`}</span>
                        </div>
                      )
                    }
                  >
                    {(c) => <CollectionTile c={c()} focused={isFocused} onEdit={() => setEditKey(c().key)} />}
                  </Show>
                )}
              />
            </div>
            <footer class="hints">
              <Hint action="select" label="Open" />
              <Hint action="favorite" label="Edit" />
              <Hint action="secondary" label="Search" />
              <Hint actions={["prevPlatform", "nextPlatform"]} label="Platform" />
              <Hint action="menu" label="Settings" />
              <Hint actions={["zoomOut", "zoomIn"]} label="Tile size" />
            </footer>
          </>
        }
      >
      <Show when={openCol()}>
        {(c) => (
          <CollectionBanner
            c={c()}
            focused={bannerFocus()}
            onEdit={() => setEditKey(c().key)}
            onBack={() => library.setPlatform("collections")}
          />
        )}
      </Show>
      <Show when={collectionError()}>
        <p class="warn collection-error">{collectionError()}</p>
      </Show>
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
                <Show
                  when={openKey() && !library.activeFilter()}
                  fallback={
                    <>
                      <h2>No matches</h2>
                      <p>
                        {library.query() ? `Nothing matches “${library.query()}”` : "No games here"}
                        {library.activeFilter() ? " with your filter" : ""}.
                      </p>
                      <div class="hints-row">
                        <Hint action="secondary" label="Search again" />
                        <Hint action="back" label={library.query() ? "Clear search" : "Clear filter"} />
                      </div>
                    </>
                  }
                >
                  <h2>{openCol()?.kind === "smart" ? "No games match these rules" : "Nothing in this collection yet"}</h2>
                  <p>{openCol()?.kind === "smart" ? "Edit the rules to find games." : "Choose Edit… then Add games… to fill this collection."}</p>
                  <div class="hints-row">
                    <Hint action="up" label="Rename / Edit" />
                    <Hint action="back" label="Collections" />
                  </div>
                </Show>
              </Show>
            </Show>
          </div>
        }
      >
        <div class="list-split" ref={splitEl} style={{ "--dock-w": library.panelWidth() ? `${library.panelWidth()}px` : undefined }}>
        <div class="grid-wrap" onWheel={onWheel}>
          <VirtualGrid
            items={games()}
            focused={focused()}
            onFocus={setFocused}
            onActivate={openSide}
            onContext={(i) => {
              const c = openCol();
              const g = games()[i];
              if (c && c.kind !== "smart" && g) void removeCollectionMember(c, g);
              else openDetails(i);
            }}
            onNavigator={(n) => (navigate = n)}
            minCardWidth={minCardWidth()}
            itemAspect={itemAspect}
            rowHeight={library.listView() ? LIST_ROW_H : undefined}
            groupOf={sectioned() ? (g) => g.platformId : undefined}
            groupLabel={sectionLabel}
            gap={library.listView() ? 2 : 18}
            renderItem={(g, isFocused) => (
              <Show
                when={library.listView()}
                fallback={
                  <GameCard
                    game={g}
                    focused={isFocused}
                    showIcon={library.platform() === "recent" || !!openKey()}
                    platformLabel={showPlatformBadge() ? shortNames().get(g().platformId) : undefined}
                  />
                }
              >
                <GameListRow game={g} focused={isFocused} platformLabel={showPlatformBadge() ? shortNames().get(g().platformId) : undefined} />
              </Show>
            )}
          />
        </div>
        <Show when={library.listView() && focusedGame()}>
          {(g) => (
            <>
            <div class="dock-resizer" title="Drag to resize the panel (double-click to reset)" onPointerDown={dragPanel} onDblClick={() => library.setPanelWidth(0)} />
            <GameSidebar docked active={sideOpen()} id={g().id} onClose={() => setSideOpen(false)} onMore={setInfoId} onSettings={props.onOpenSettings} />
            </>
          )}
        </Show>
        </div>
        <footer class="hints">
          <Hint action="select" label="Open" />
          <Hint action="favorite" label={openCol() && openCol()!.kind !== "smart" ? "Remove from collection" : "Details"} />
          <Hint action="secondary" label="Search" />
          <Hint action="filter" label="Filter" />
          <Hint actions={["prevPlatform", "nextPlatform"]} label="Platform" />
          <Hint actions={["prevLetter", "nextLetter"]} label="Letter" />
          <Hint action="menu" label="Settings" />
          <Hint action="toggleTv" label="TV Mode" />
          <Hint actions={["zoomOut", "zoomIn"]} label={library.listView() ? "Panel width" : "Card size"} />
          <Hint action="toggleGroups" label={library.listView() ? "Grid" : "List"} />
          <input
            hidden={library.listView()}
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
          <span class="zoom-pct" hidden={library.listView()} title="Card size (100% = the default)">
            {Math.round((library.cardSize() / library.CARD_DEFAULT) * 100)}%
          </span>
          <span class="hint-count">
            {focused() + 1} / {games().length}
          </span>
        </footer>
      </Show>
      </Show>

      </div>
      <Show when={library.listView() ? null : sideId()}>
        {(id) => (
          <GameSidebar
            id={id()}
            onClose={() => setSideOpen(false)}
            onMore={setInfoId}
            onSettings={props.onOpenSettings}
          />
        )}
      </Show>
      </div>

      <Show when={infoId()} keyed>
        {(id) => <GameInfo id={id} onClose={() => setInfoId(null)} />}
      </Show>
      <Show when={editing()}>
        {(c) => (
          <CollectionEditor
            c={c()}
            onClose={() => setEditKey(null)}
            onDeleted={() => {
              lastTile = 0;
              library.setPlatform("collections");
            }}
          />
        )}
      </Show>
      <Show when={filterOpen()}>
        <FilterPanel
          initial={library.filter()}
          onChange={(r) => library.setFilter(r)}
          onSaveAs={(r) =>
            saveSmartCollection(r, (key) => {
              setFilterOpen(false);
              openCollection(key);
            })
          }
          onClose={() => setFilterOpen(false)}
        />
      </Show>
    </div>
  );
}
