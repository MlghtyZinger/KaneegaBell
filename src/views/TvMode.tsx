import { createEffect, createMemo, createResource, createSignal, For, on, onCleanup, Show } from "solid-js";
import { TvDescription } from "./TvDescription";
import { api, artUrl, errorText, type Collection, type GameRow } from "../api";
import { collectionKeyOf, library, type PlatformFilter } from "../state/library";
import { collections } from "../state/collections";
import { formatLastPlayed, formatPlayTime } from "../state/session";
import { osk } from "../state/osk";
import { pushHandler, type Action } from "../input/actions";
import { GameInfo } from "./GameInfo";
import { NavScope } from "../components/NavScope";
import { Hint } from "../components/Hints";
import { ConsoleMenus, type ConsoleEntry } from "./ConsoleMenus";
import { fx } from "../state/fx";
import { Model, caseOf } from "../fx/Model";
import { launchAnimation, launchDone } from "../fx/LaunchFx";
import "../styles/tv.css";
import { sfx } from "../fx/audio";
import { showcaseScale, showcaseWindow } from "./tvShowcase";

/** Wait this long after the last move before swapping the background (holding a direction stays smooth). */
const SETTLE_MS = 160;
/** Text size presets (Settings → General → TV Mode). */
export const TV_TEXT: Record<string, number> = { s: 0.85, m: 1, l: 1.2, xl: 1.4 };

function initial(title: string): string {
  const c = title.replace(/^(the|a|an)\s+/i, "").charAt(0).toUpperCase();
  return c >= "A" && c <= "Z" ? c : "#";
}

interface Layer {
  key: number;
  url: string;
  /** No hero: the cover, blurred and scaled up. */
  blur: boolean;
}

/** Showcase TV Mode: logo navigation, one large case, and a separate game-information column. */
export function TvMode(props: { onExit: () => void; onOpenSettings: () => void }) {
  const games = library.visibleGames;
  const { focused, setFocused } = library;
  const [infoId, setInfoId] = createSignal<number | null>(null);
  const [menuOpen, setMenuOpen] = createSignal(false);
  const [stylesOpen, setStylesOpen] = createSignal(false);
  const menuStyle = () => { const value = fx.get("tvMenu"); return value === "wii" || value === "ps3" ? value : "kb"; };
  const styleName = () => ({ kb: "KB", wii: "Wii", ps3: "PS3" })[menuStyle()];
  const changeStyle = (value: "kb" | "wii" | "ps3") => {
    fx.set("tvMenu", value); setStylesOpen(false); setZone("games");
    drag = null; setInsp({ ry: 0, rx: 0, flipped: false, spring: true });
  };
  const [platformsOpen, setPlatformsOpen] = createSignal(false);
  const [zone, setZone] = createSignal<"games" | "actions" | "filter">("games");
  const [actionIndex, setActionIndex] = createSignal(0);
  const [launching, setLaunching] = createSignal(false);
  const [launchError, setLaunchError] = createSignal<string | null>(null);
  // The Collections tab lists collections instead of games; opening one is the "col:<key>" tab.
  const overview = () => library.platform() === "collections" && !library.query();
  const cols = collections.visible;
  const [colFocused, setColFocused] = createSignal(0);
  const currentCol = () => (overview() ? cols()[colFocused()] : undefined);
  // The overview's background: the focused collection's first game with a background, else a cover.
  const colGame = () => {
    const c = currentCol();
    if (!c) return undefined;
    const m = collections.gamesOf(c);
    return m.find((g) => g.hero) ?? m.find((g) => g.art > 0);
  };
  const current = () => (overview() ? colGame() : (games()[focused()] as GameRow | undefined));
  const count = () => (overview() ? cols().length : games().length);
  const KIND = { mine: "Collection", smart: "Smart collection", series: "Series", franchise: "Franchise" } as const;

  // Clamp when the list shrinks; new tab or search starts at the first game.
  createEffect(on([library.platform, library.query, library.filterKey], () => setFocused(0), { defer: true }));
  createEffect(() => {
    const n = games().length;
    if (focused() >= n) setFocused(Math.max(0, n - 1));
    if (colFocused() >= cols().length) setColFocused(Math.max(0, cols().length - 1));
  });

  const railWindow = createMemo(() => showcaseWindow(overview() ? colFocused() : focused(), count()));
  const shelf3d = () => fx.shelf3d() && !overview() && menuStyle() === "kb";
  const [caseSize, setCaseSize] = createSignal({ width: 400, height: 600 });
  const caseScale = () => {
    const g = current();
    if (!g) return 1;
    const [w, h] = caseOf(g.platformId).size;
    return showcaseScale(caseSize().width, caseSize().height, w, h, g.art > 0 ? g.artAspect : null);
  };
  // Inspecting the focused case: turn it (right stick, [ ], or drag it), tilt it (right stick up/down), flip it (R3 / V).
  const [insp, setInsp] = createSignal({ ry: 0, rx: 0, flipped: false, spring: true });
  createEffect(on(focused, () => setInsp({ ry: 0, rx: 0, flipped: false, spring: true }), { defer: true }));
  const inspStyle = () => {
    const v = insp();
    return { transform: `rotateX(${v.rx}deg) rotateY(${v.ry + (v.flipped ? 180 : 0)}deg)`, "--ry": String(v.ry), transition: v.spring ? "transform .5s cubic-bezier(.2,.8,.2,1)" : "none" };
  };
  const turn = (dy: number, dx = 0) =>
    setInsp((v) => ({ ...v, spring: true, ry: Math.max(-80, Math.min(80, v.ry + dy)), rx: Math.max(-25, Math.min(25, v.rx + dx)) }));
  let drag: { x: number; y: number; ry: number; rx: number; moved: boolean } | null = null;
  let dragged = false;
  const onDragMove = (e: PointerEvent) => {
    if (!drag) return;
    const dx = e.clientX - drag.x;
    const dy = e.clientY - drag.y;
    if (Math.abs(dx) + Math.abs(dy) > 5) drag.moved = true;
    setInsp((v) => ({ ...v, spring: false, ry: Math.max(-80, Math.min(80, drag!.ry + dx * 0.6)), rx: Math.max(-25, Math.min(25, drag!.rx - dy * 0.3)) }));
  };
  const onDragEnd = () => {
    if (!drag) return;
    dragged = drag.moved;
    drag = null;
    setInsp((v) => ({ ...v, spring: true, ry: 0, rx: 0 }));
  };
  addEventListener("pointermove", onDragMove);
  addEventListener("pointerup", onDragEnd);
  onCleanup(() => {
    removeEventListener("pointermove", onDragMove);
    removeEventListener("pointerup", onDragEnd);
  });

  const caseObserver = new ResizeObserver(([entry]) => setCaseSize({ width: entry.contentRect.width, height: entry.contentRect.height }));
  onCleanup(() => caseObserver.disconnect());

  // ---- background: crossfade to the focused game's hero once focus settles.
  const [layers, setLayers] = createSignal<Layer[]>([]);
  const [settled, setSettled] = createSignal<GameRow | undefined>(undefined);
  const [settledCol, setSettledCol] = createSignal<Collection | undefined>(undefined);
  let settleTimer: ReturnType<typeof setTimeout> | undefined;
  createEffect(
    on([current, currentCol], ([g, c]) => {
      clearTimeout(settleTimer);
      settleTimer = setTimeout(() => {
        setSettled(g);
        setSettledCol(c);
      }, SETTLE_MS);
    }),
  );
  onCleanup(() => clearTimeout(settleTimer));
  let layerKey = 0;
  createEffect(
    on([settled, settledCol, menuStyle], ([g, c]) => {
      if (menuStyle() !== "kb") { setLayers([]); return; }
      const own = c && collections.ownHero(c);
      if (!g && !own) return;
      const layer: Layer | null = own
        ? { key: ++layerKey, url: own, blur: false }
        : !g
          ? null
          : g.hero
        ? { key: ++layerKey, url: artUrl(g.id, g.hero, "h"), blur: false }
        : g.art > 0
          ? { key: ++layerKey, url: artUrl(g.id, g.art, "f"), blur: true }
          : null;
      if (!layer) {
        setLayers([]);
        return;
      }
      if (layers().at(-1)?.url === layer.url) return;
      // Keep the previous layer underneath until the new one has faded in.
      setLayers((ls) => [...ls.slice(-1), layer]);
      const key = layer.key;
      setTimeout(() => setLayers((ls) => (ls.at(-1)?.key === key ? ls.slice(-1) : ls)), 900);
    }),
  );

  // Details for the settled game (year, developer, genres, description).
  const [detail] = createResource(
    () => settled()?.id,
    (id) => api.getGame(id).catch(() => null),
  );
  const metaLine = () => {
    const d = detail();
    if (!d || d.id !== settled()?.id) return "";
    const m = d.meta;
    const year = m.releaseDate ? new Date(m.releaseDate * 1000).getUTCFullYear() : null;
    return [year, m.developer, ...m.genres.slice(0, 3)].filter(Boolean).join("  ·  ");
  };
  // A logo file that's gone (cache cleared): show the title instead.
  const [logoFailed, setLogoFailed] = createSignal<ReadonlySet<number>>(new Set());
  const platformName = (id: string) => library.platforms().find((p) => p.id === id)?.name ?? id.toUpperCase();

  // ---- input
  function jumpLetter(dir: 1 | -1) {
    const list = games();
    if (overview() || list.length === 0) return;
    const cur = initial(list[focused()].title);
    if (dir > 0) {
      const i = list.findIndex((g, k) => k > focused() && initial(g.title) !== cur);
      if (i >= 0) setFocused(i);
    } else {
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

  async function toggleFavorite() {
    const g = current();
    if (!g) return;
    await api.setFavorite(g.id, !g.favorite);
    await library.reload();
  }

  const step = (d: number) => {
    const n = count();
    if (n === 0) return;
    if (overview()) setColFocused(Math.min(n - 1, Math.max(0, colFocused() + d)));
    else setFocused(Math.min(n - 1, Math.max(0, focused() + d)));
  };

  const onAction = (a: Action): boolean => {
    if (a === "filter") { setStylesOpen(true); return true; }
    switch (a) {
      case "left":
        if (zone() === "actions" && actionIndex() > 0) setActionIndex(0); else setZone("games");
        return true;
      case "right":
        if (!overview()) { if (zone() === "actions") setActionIndex(1); else { setZone("actions"); setActionIndex(0); } }
        return true;
      case "up":
        if (zone() === "actions") setZone("games");
        else if ((overview() ? colFocused() : focused()) === 0) setZone("filter"); else step(-1);
        return true;
      case "down":
        if (zone() === "filter") setZone("games"); else if (zone() === "games") step(1);
        return true;
      case "prevPlatform": library.cyclePlatform(-1, library.platformTabs()); setZone("games"); return true;
      case "nextPlatform": library.cyclePlatform(1, library.platformTabs()); setZone("games"); return true;
      case "prevLetter": jumpLetter(-1); return true;
      case "nextLetter": jumpLetter(1); return true;
      case "favorite": if (!overview()) void toggleFavorite(); return true;
      case "secondary": openSearch(); return true;
      case "select": {
        if (zone() === "filter") { setPlatformsOpen(true); return true; }
        if (!overview() && zone() === "games") { setZone("actions"); setActionIndex(0); return true; }
        if (!overview() && actionIndex() === 0) { void playCurrent(); return true; }
        if (overview()) {
          const c = currentCol();
          if (c) library.setPlatform(`col:${c.key}`);
          return true;
        }
        const g = current();
        if (g) setInfoId(g.id);
        return true;
      }
      case "back":
        if (zone() !== "games") { setZone("games"); return true; }
        if (library.query()) library.setQuery("");
        else if (library.activeFilter()) library.clearFilter();
        else if (collectionKeyOf(library.platform())) library.setPlatform("collections");
        else return false; // nothing left to close: Esc falls through to the menu (actions.ts)
        return true;
      case "turnLeft": if (shelf3d()) turn(-20); return true;
      case "turnRight": if (shelf3d()) turn(20); return true;
      case "zoomIn": if (shelf3d()) turn(0, 10); return true;
      case "zoomOut": if (shelf3d()) turn(0, -10); return true;
      case "toggleGroups":
        if (shelf3d()) {
          setInsp((v) => ({ ...v, spring: true, ry: 0, flipped: !v.flipped }));
          sfx("select");
        }
        return true;
      case "menu": setMenuOpen(true); return true;
      case "toggleTv": props.onExit(); return true;
      default: return true;
    }
  };
  onCleanup(pushHandler(onAction));

  const onWheel = (e: WheelEvent) => {
    if (!menuOpen() && !stylesOpen() && !platformsOpen() && !infoId() && (e.deltaY !== 0 || e.deltaX !== 0)) step((e.deltaY || e.deltaX) > 0 ? 1 : -1);
  };

  const tabLabel = (id: PlatformFilter) => {
    if (id === "all") return "All platforms";
    if (id === "favorites") return "Favorites";
    if (id === "recent") return "Recently played";
    if (id === "collections") return "Collections";
    return library.platforms().find((x) => x.id === id)?.shortName ?? id;
  };

  async function playCurrent() {
    const g = current();
    if (!g || launching() || overview()) return;
    setLaunching(true); setLaunchError(null);
    try { if (await launchAnimation(g.id)) await api.launchGame(g.id); }
    catch (e) { launchDone(); setLaunchError(errorText(e)); }
    finally { setLaunching(false); }
  }
  const platformCount = (id: PlatformFilter) => id === "all" ? library.games().length : id === "favorites" ? library.favorites().length : id === "recent" ? library.recent().length : id === "collections" ? cols().length : library.platforms().find((p) => p.id === id)?.count ?? 0;

  const consoleEntry = (index: number): ConsoleEntry | undefined => {
    if (overview()) {
      const c = cols()[index];
      if (!c) return undefined;
      const picture = collections.chosenPicture(c)?.url ?? collections.autoLogo(c) ?? undefined;
      return { key: "col:" + c.key, title: c.name, subtitle: collections.gamesOf(c).length + " games", logo: picture, picture, banner: collections.ownHero(c) ?? undefined };
    }
    const g = games()[index];
    if (!g) return undefined;
    const banner = g.screenshot ? artUrl(g.id, g.screenshot, "q") : g.hero ? artUrl(g.id, g.hero, "h") : undefined;
    return { key: String(g.id), title: g.title, subtitle: platformName(g.platformId), logo: g.logo ? artUrl(g.id, g.logo, "l") : undefined,
      picture: g.art ? artUrl(g.id, g.art, "t") : banner, banner };
  };

  // Settings → General → TV Mode: text size and the overscan margin (for TVs
  // that cut off the picture's edges).
  const [textSize] = createResource(() => api.getSetting<string>("tvTextSize").catch(() => null));
  const [overscan] = createResource(() => api.getSetting<number>("tvOverscan").catch(() => null));

  return (
    <div
      class={`tv tv-showcase tv-style-${menuStyle()}`}
      style={{ "--tv-text": String(TV_TEXT[textSize() ?? "m"] ?? 1), "--overscan": String(overscan() ?? 0) }}
      onWheel={onWheel}
    >
      <div class="tv-bg" aria-hidden="true">
        <For each={layers()}>
          {(l) => (
            <div class="tv-bg-layer" classList={{ blur: l.blur }}>
              <img src={l.url} alt="" decoding="async" onError={(e) => (e.currentTarget.style.visibility = "hidden")} />
            </div>
          )}
        </For>
      </div>
      <div class="tv-shade" aria-hidden="true" />



      <button class="tv-style-switch" onClick={() => setStylesOpen(true)} aria-label="Change TV menu style">Menu style · {styleName()} ▾</button>
      <Show when={menuStyle() === "kb"}>
      <main class="tv-main">
        <nav class="tv-rail" aria-label="Game library">
          <button class="tv-filter" classList={{ "nav-focus": zone() === "filter" }} onClick={() => setPlatformsOpen(true)}>
            <span>Platform filter</span><strong>{tabLabel(collectionKeyOf(library.platform()) ? "collections" : library.platform())}</strong><span aria-hidden="true">▾</span>
          </button>
          <Show when={library.query()}><p class="tv-search-label">Search: {library.query()}</p></Show>
          <div class="tv-rail-games">
            <For each={railWindow()}>{(i) => (
              <Show when={!overview()} fallback={
                <Show when={cols()[i]}>{(c) => (
                  <button class="tv-rail-item" classList={{ active: i === colFocused(), "nav-focus": i === colFocused() && zone() === "games" }} onClick={() => {
                    if (i === colFocused()) library.setPlatform("col:" + c().key); else setColFocused(i);
                    setZone("games");
                  }}>
                    <Show when={collections.chosenPicture(c())?.url ?? collections.autoLogo(c())} fallback={<span>{c().name}</span>}>{(url) => <img src={url()} alt={c().name} decoding="async" />}</Show>
                    <small>{collections.gamesOf(c()).length} games</small>
                  </button>
                )}</Show>
              }>
                <Show when={games()[i]}>{(g) => (
                  <button class="tv-rail-item" classList={{ active: i === focused(), "nav-focus": i === focused() && zone() === "games" }} aria-label={g().title} onClick={() => {
                    if (i === focused()) { setZone("actions"); setActionIndex(0); } else { setFocused(i); setZone("games"); }
                  }}>
                    <Show when={g().logo && !logoFailed().has(g().id)} fallback={<span>{g().title}</span>}>
                      <img src={artUrl(g().id, g().logo!, "l")} alt={g().title} decoding="async" onError={() => setLogoFailed((ids) => new Set(ids).add(g().id))} />
                    </Show>
                    <small>{platformName(g().platformId)}</small>
                  </button>
                )}</Show>
              </Show>
            )}</For>
          </div>
          <div class="tv-rail-arrows"><button aria-label="Previous game" onClick={() => { setZone("games"); step(-1); }}>▲</button><button aria-label="Next game" onClick={() => { setZone("games"); step(1); }}>▼</button></div>
        </nav>
        <div class="tv-case-area" ref={(el) => { caseObserver.observe(el); onCleanup(() => caseObserver.unobserve(el)); }}>
          <Show when={current()} keyed>{(g) => (
            <Show when={shelf3d()} fallback={
              <Show when={g.art > 0} fallback={<span class="tv-case-empty">{currentCol()?.name ?? g.title}</span>}><img class="tv-flat-cover" src={artUrl(g.id, g.art, "f")} alt={g.title} decoding="async" /></Show>
            }>
              <div class="tv-case-stage" onPointerDown={(e) => { const v = insp(); drag = { x: e.clientX, y: e.clientY, ry: v.ry, rx: v.rx, moved: false }; }} onClick={() => {
                if (dragged) { dragged = false; return; } setZone("actions"); setActionIndex(0);
              }}>
                <div class="m3-turn" style={inspStyle()}><Model game={g} platformName={platformName(g.platformId)} scale={caseScale()} detail /></div>
              </div>
            </Show>
          )}</Show>
          <Show when={shelf3d()}><div class="tv-case-controls">
            <button onClick={() => turn(-20)} aria-label="Turn case left">↶</button><button onClick={() => turn(20)} aria-label="Turn case right">↷</button>
            <button onClick={() => setInsp((v) => ({ ...v, spring: true, flipped: !v.flipped }))}>Flip</button>
          </div></Show>
        </div>
        <section class="tv-info" aria-live="polite">
          <Show when={currentCol()} keyed>{(c) => (
            <div class="tv-info-inner"><p class="tv-platform">{KIND[c.kind]}</p><h1 class="tv-title">{c.name}</h1><p class="tv-meta">{collections.gamesOf(c).length} games</p><p class="tv-play">{collections.statsLine(c)}</p><button class="tv-more" onClick={() => library.setPlatform("col:" + c.key)}>Open collection</button></div>
          )}</Show>
          <Show when={!overview() && current()}>{(g) => (
            <div class="tv-info-inner">
              <p class="tv-platform">{platformName(g().platformId)}</p><h1 class="tv-title">{g().title}</h1>
              <Show when={g().variant}><p class="tv-variant">{g().variant}</p></Show>
              <p class="tv-meta">{settled()?.id === g().id ? metaLine() : ""}</p>
              <p class="tv-play">{g().favorite ? "★ Favorite · " : ""}{g().playSeconds > 0 ? formatPlayTime(g().playSeconds) + " played · " + formatLastPlayed(g().lastPlayed) : "Never played"}</p>
              <Show when={detail()?.id === g().id && detail()?.meta.description} fallback={<p class="tv-desc tv-desc-empty">{detail.loading ? "Loading description…" : "No description available."}</p>}>
                <TvDescription text={detail()!.meta.description!} lines={textSize() === "xl" ? 3 : 4} />
              </Show>
              <div class="tv-actions"><button class="tv-play-button" classList={{ "nav-focus": zone() === "actions" && actionIndex() === 0 }} disabled={launching()} onClick={() => void playCurrent()}>{launching() ? "Starting…" : "▶ Play"}</button><button class="tv-more" classList={{ "nav-focus": zone() === "actions" && actionIndex() === 1 }} onClick={() => setInfoId(g().id)}>More…</button></div>
              <Show when={launchError()}><p class="warn">{launchError()}</p></Show>
              <Show when={g().screenshot || g().hero}>
                <div class="tv-artwork" classList={{ "is-screenshot": !!g().screenshot }}>
                  <img class="tv-artwork-fill" src={artUrl(g().id, (g().screenshot || g().hero)!, g().screenshot ? "q" : "h")} alt="" decoding="async" />
                  <img class="tv-artwork-picture" src={artUrl(g().id, (g().screenshot || g().hero)!, g().screenshot ? "q" : "h")} alt={g().screenshot ? g().title + " gameplay screenshot" : g().title + " artwork"} decoding="async" />
                </div>
              </Show>
            </div>
          )}</Show>
          <Show when={count() === 0}><p class="tv-empty">{library.loaded() ? overview() ? "No collections yet." : "No games here." : "Loading…"}</p></Show>
        </section>
      </main>

      <footer class="tv-hints">
        <Hint action="select" label={overview() ? "Open" : zone() === "games" ? "Actions" : zone() === "filter" ? "Platforms" : actionIndex() === 0 ? "Play" : "More…"} />
        <Hint action="secondary" label="Search" />
        <Hint action="filter" label="Menu style" />
        <Show when={!overview()}>
          <Hint action="favorite" label="Favorite" />
        </Show>
        <Show when={shelf3d()}>
          <Hint actions={["turnLeft", "turnRight"]} label="Turn" />
          <Hint action="toggleGroups" label="Flip" />
        </Show>
        <Hint actions={["prevPlatform", "nextPlatform"]} label="Platform" />
        <button class="hint-btn" onClick={() => setMenuOpen(true)}><Hint action="menu" label="Menu" /></button>
        {/* Clickable too, for the mouse. */}
        <button class="hint-btn" onClick={props.onExit} title="Back to the desktop view">
          <Hint action="toggleTv" label="Desktop mode" />
        </button>
        <span class="tv-count">
          {count() > 0 ? `${(overview() ? colFocused() : focused()) + 1} / ${count()}` : ""}
        </span>
      </footer>
      </Show>
      <Show when={menuStyle() !== "kb"}>
        <ConsoleMenus style={menuStyle() === "wii" ? "wii" : "ps3"} count={count()} focused={overview() ? colFocused() : focused()} entry={consoleEntry}
          select={(index) => { if (index < 0) return; if (overview()) setColFocused(index); else setFocused(index); }}
          play={() => { if (overview()) { const c = currentCol(); if (c) library.setPlatform("col:" + c.key); } else void playCurrent(); }}
          more={() => { const g = current(); if (g) setInfoId(g.id); }}
          platforms={() => setPlatformsOpen(true)} styles={() => setStylesOpen(true)} settings={props.onOpenSettings} search={openSearch}
          launching={launching()} error={launchError()} />
      </Show>

      <Show when={stylesOpen()}>
        <div class="overlay tv-menu-overlay" onClick={(e) => e.target === e.currentTarget && setStylesOpen(false)}>
          <NavScope class="tv-menu tv-style-picker" modal onBack={() => setStylesOpen(false)}>
            <h2>TV menu style</h2>
            <For each={["kb", "wii", "ps3"] as const}>{(value) => <button class="tv-menu-item" data-autofocus={menuStyle() === value ? "" : undefined} aria-pressed={menuStyle() === value} onClick={() => changeStyle(value)}><strong>{{ kb: "KB · KaneegaBell", wii: "Wii", ps3: "PS3" }[value]}</strong><small>{{ kb: "Large case, logo list and gameplay preview", wii: "Twelve channels per page", ps3: "XMB categories, game list and options" }[value]}</small></button>}</For>
          </NavScope>
        </div>
      </Show>
      <Show when={menuOpen()}>
        <TvMenu
          onClose={() => setMenuOpen(false)}
          onStyle={() => { setMenuOpen(false); setStylesOpen(true); }}
          onPlatforms={() => {
            setMenuOpen(false);
            setPlatformsOpen(true);
          }}
          onSearch={() => {
            setMenuOpen(false);
            openSearch();
          }}
          onSettings={() => {
            setMenuOpen(false);
            props.onOpenSettings();
          }}
          onDesktop={() => {
            setMenuOpen(false);
            props.onExit();
          }}
        />
      </Show>
      <Show when={platformsOpen()}>
        <div class="overlay tv-menu-overlay" onClick={(e) => e.target === e.currentTarget && setPlatformsOpen(false)}>
          <NavScope class="tv-menu tv-platform-picker" modal onBack={() => { setPlatformsOpen(false); setZone("games"); }}>
            <h2>Platforms</h2>
            <div class="tv-platform-grid">
              <For each={library.platformTabs()}>
                {(id) => {
                  const active = () => library.platform() === id || (id === "collections" && !!collectionKeyOf(library.platform()));
                  return (
                    <button class="tv-menu-item" classList={{ active: active() }} data-autofocus={active() ? "" : undefined} onClick={() => {
                      library.setQuery("");
                      library.setPlatform(id);
                      setPlatformsOpen(false);
                      setZone("games");
                    }}><span>{tabLabel(id)}</span><small>{platformCount(id)} {id === "collections" ? "collections" : "games"}</small></button>
                  );
                }}
              </For>
            </div>
          </NavScope>
        </div>
      </Show>
      <Show when={infoId()} keyed>
        {(id) => <GameInfo id={id} onClose={() => setInfoId(null)} />}
      </Show>
    </div>
  );
}

function TvMenu(props: { onStyle: () => void; onClose: () => void; onPlatforms: () => void; onSearch: () => void; onSettings: () => void; onDesktop: () => void }) {
  const [confirmOff, setConfirmOff] = createSignal(false);
  return (
    <div class="overlay tv-menu-overlay" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <NavScope class="tv-menu" modal onBack={props.onClose}>
        <button class="tv-menu-item" data-autofocus onClick={props.onClose}>
          Resume
        </button>
        <button class="tv-menu-item" onClick={props.onStyle}>Menu style…</button>
        <button class="tv-menu-item" onClick={props.onPlatforms}>
          Platforms…
        </button>
        <button class="tv-menu-item" onClick={props.onSearch}>
          Search
        </button>
        <button class="tv-menu-item" onClick={props.onSettings}>
          Settings
        </button>
        <button class="tv-menu-item" onClick={props.onDesktop}>
          Desktop mode
        </button>
        <button class="tv-menu-item" onClick={() => (confirmOff() ? void api.shutdownPc().catch(() => {}) : setConfirmOff(true))}>
          {confirmOff() ? "Press again to turn off the PC" : "Turn off PC"}
        </button>
      </NavScope>
    </div>
  );
}
