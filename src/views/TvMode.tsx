import { createEffect, createMemo, createResource, createSignal, For, on, onCleanup, Show } from "solid-js";
import { lead } from "../state/summary";
import { api, artUrl, type Collection, type GameRow } from "../api";
import { collectionKeyOf, library, type PlatformFilter } from "../state/library";
import { collections } from "../state/collections";
import { formatLastPlayed, formatPlayTime } from "../state/session";
import { osk } from "../state/osk";
import { pushHandler, type Action } from "../input/actions";
import { GameInfo } from "./GameInfo";
import { NavScope } from "../components/NavScope";
import { Hint } from "../components/Hints";

/** Cards either side of the focused one that exist in the DOM. */
const WINDOW_BEFORE = 10;
const WINDOW_AFTER = 18;
/** Wait this long after the last move before swapping the background (holding a direction stays smooth). */
const SETTLE_MS = 160;
const DEFAULT_ASPECT = 1.4;
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

/**
 * TV Mode: a couch view in the spirit of Steam's Big Picture. The focused
 * game's SteamGridDB hero fills the screen (crossfading, drifting slowly;
 * a blurred cover when there's no hero), with its logo or title, details and
 * play time over it, and a shelf of covers along the bottom. Same library,
 * tabs, search and filters as desktop mode, and the same focus.
 */
export function TvMode(props: { onExit: () => void; onOpenSettings: () => void }) {
  const games = library.visibleGames;
  const { focused, setFocused } = library;
  const [infoId, setInfoId] = createSignal<number | null>(null);
  const [menuOpen, setMenuOpen] = createSignal(false);
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

  // ---- shelf geometry: every card shares a height; widths follow the cover.
  const platformAspect = createMemo(() => new Map(library.platforms().map((p) => [p.id, p.artAspect ?? DEFAULT_ASPECT])));
  const aspect = (g: GameRow) => {
    const a = (g.art > 0 ? g.artAspect : null) ?? platformAspect().get(g.platformId) ?? DEFAULT_ASPECT;
    // Up to PSP boxes (~1.72): cropping their top band looks broken.
    return Math.min(1.8, Math.max(0.6, a));
  };
  const [shelfH, setShelfH] = createSignal(320);
  const offsets = createMemo(() => {
    const list = games();
    const h = shelfH();
    // Room for the focused card to grow (scale 1.14) without touching its neighbors.
    const GAP = Math.round(h * 0.14);
    const x = new Float64Array((overview() ? cols().length : list.length) + 1);
    // Collection tiles are square.
    for (let i = 0; i < x.length - 1; i++) x[i + 1] = x[i] + (overview() ? h : h / aspect(list[i])) + GAP;
    return x;
  });
  const windowed = createMemo(() => {
    const n = overview() ? cols().length : games().length;
    const at = overview() ? colFocused() : focused();
    const from = Math.max(0, at - WINDOW_BEFORE);
    const to = Math.min(n, at + WINDOW_AFTER);
    return Array.from({ length: to - from }, (_, k) => from + k);
  });
  let shelfEl!: HTMLDivElement;
  const measure = () => shelfEl && setShelfH(Math.round(shelfEl.clientHeight * 0.78));
  const ro = new ResizeObserver(measure);
  onCleanup(() => ro.disconnect());

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
    on([settled, settledCol], ([g, c]) => {
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
    switch (a) {
      case "left": step(-1); return true;
      case "right": step(1); return true;
      case "up":
      case "down":
        return true;
      case "prevPlatform": library.cyclePlatform(-1, library.platformTabs()); return true;
      case "nextPlatform": library.cyclePlatform(1, library.platformTabs()); return true;
      case "prevLetter": jumpLetter(-1); return true;
      case "nextLetter": jumpLetter(1); return true;
      case "favorite": if (!overview()) void toggleFavorite(); return true;
      case "secondary": openSearch(); return true;
      case "select": {
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
        if (library.query()) library.setQuery("");
        else if (library.activeFilter()) library.clearFilter();
        else if (collectionKeyOf(library.platform())) library.setPlatform("collections");
        return true;
      case "menu": setMenuOpen(true); return true;
      case "toggleTv": props.onExit(); return true;
      default: return true;
    }
  };
  onCleanup(pushHandler(onAction));

  const onWheel = (e: WheelEvent) => {
    if (e.deltaY !== 0 || e.deltaX !== 0) step((e.deltaY || e.deltaX) > 0 ? 1 : -1);
  };

  const tabLabel = (id: PlatformFilter) => {
    if (id === "all") return "All games";
    if (id === "favorites") return "Favorites";
    if (id === "recent") return "Recently played";
    if (id === "collections") return "Collections";
    return library.platforms().find((x) => x.id === id)?.shortName ?? id;
  };

  // A clock, as consoles have (updated twice a minute).
  const [now, setNow] = createSignal(new Date());
  const clock = setInterval(() => setNow(new Date()), 30_000);
  onCleanup(() => clearInterval(clock));

  // Settings → General → TV Mode: text size and the overscan margin (for TVs
  // that cut off the picture's edges).
  const [textSize] = createResource(() => api.getSetting<string>("tvTextSize").catch(() => null));
  const [overscan] = createResource(() => api.getSetting<number>("tvOverscan").catch(() => null));

  return (
    <div
      class="tv"
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

      <header class="tv-top">
        <nav class="tv-tabs" aria-label="Platforms">
          <Show
            when={!library.query()}
            fallback={<span class="tv-tab active">Search: “{library.query()}”</span>}
          >
            <For each={library.platformTabs()}>
              {(id) => (
                <button class="tv-tab" classList={{ active: library.platform() === id || (id === "collections" && !!collectionKeyOf(library.platform())) }} onClick={() => library.setPlatform(id)}>
                  {tabLabel(id)}
                </button>
              )}
            </For>
          </Show>
        </nav>
        <span class="tv-clock">{now().toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}</span>
      </header>

      <Show when={currentCol()} keyed>
        {(c) => {
          const pic = () => collections.chosenPicture(c);
          const logo = () => (pic()?.kind === "logo" ? pic()!.url : !pic() ? collections.autoLogo(c) : null);
          return (
            <section class="tv-info" aria-live="polite">
              <div class="tv-info-inner">
                <Show when={logo()} fallback={<h1 class="tv-title">{c.name}</h1>}>
                  <img class="tv-logo" src={logo()!} alt={c.name} decoding="async" />
                </Show>
                <p class="tv-meta">
                  <span class="tv-platform">{KIND[c.kind]}</span>
                  <span>  ·  {collections.gamesOf(c).length} games</span>
                </p>
                <Show when={collections.statsLine(c)}>
                  <p class="tv-play">{collections.statsLine(c)}</p>
                </Show>
              </div>
            </section>
          );
        }}
      </Show>
      <Show
        when={!overview() && current()}
        fallback={
          <div class="tv-empty" hidden={overview() && cols().length > 0}>
            <h1>{overview() ? "No collections yet. Make one on the desktop." : library.loaded() ? (library.query() ? `Nothing matches “${library.query()}”` : "No games here") : "Loading…"}</h1>
          </div>
        }
      >
        {(g) => (
          <section class="tv-info" aria-live="polite">
            <Show when={settled()?.id === g().id} keyed>
              {/* Re-mounts per game, replaying the entrance animation. */}
              <div class="tv-info-inner">
                <Show when={g().logo && !logoFailed().has(g().id)} fallback={<h1 class="tv-title">{g().title}</h1>}>
                  <img
                    class="tv-logo"
                    src={artUrl(g().id, g().logo!, "l")}
                    alt={g().title}
                    decoding="async"
                    onError={() => setLogoFailed((s) => new Set(s).add(g().id))}
                  />
                </Show>
                <Show when={g().variant}>
                  <p class="tv-variant">{g().variant}</p>
                </Show>
                <p class="tv-meta">
                  <span class="tv-platform">{platformName(g().platformId)}</span>
                  <Show when={metaLine()}>
                    <span>  ·  {metaLine()}</span>
                  </Show>
                </p>
                <p class="tv-play">
                  {g().favorite ? "★ Favorite  ·  " : ""}
                  {g().playSeconds > 0
                    ? `${formatPlayTime(g().playSeconds)} played  ·  last played ${formatLastPlayed(g().lastPlayed)}`
                    : "Never played"}
                </p>
                <Show when={detail()?.id === g().id && detail()?.meta.description}>
                  <p class="tv-desc">{brief(detail()!.meta.description!)}</p>
                </Show>
              </div>
            </Show>
          </section>
        )}
      </Show>

      <div
        class="tv-shelf"
        ref={(el) => {
          shelfEl = el;
          ro.observe(el);
          queueMicrotask(measure);
        }}
      >
        <Show when={overview() && cols().length > 0}>
          <div class="tv-track" style={{ transform: `translate3d(${-(offsets()[colFocused()] ?? 0)}px, 0, 0)`, height: `${shelfH()}px` }}>
            <For each={windowed()}>
              {(i) => {
                const c = () => cols()[i];
                const cover = () => {
                  const p = c() && collections.chosenPicture(c()!);
                  return p?.kind === "cover" ? p.url : c() ? collections.mosaic(c()!)[0] : undefined;
                };
                return (
                  <Show when={c()}>
                    <div
                      class="tv-card"
                      classList={{ focused: i === colFocused(), "has-art": !!cover() }}
                      style={{ width: `${shelfH()}px`, height: `${shelfH()}px`, transform: `translate3d(${offsets()[i]}px, 0, 0)` }}
                      onClick={() => (i === colFocused() ? library.setPlatform(`col:${c()!.key}`) : setColFocused(i))}
                    >
                      <div class="tv-card-face">
                        <Show when={cover()} fallback={<span class="tv-card-title">{c()!.name}</span>}>
                          <img src={cover()!} alt="" loading="lazy" decoding="async" draggable={false} />
                        </Show>
                      </div>
                    </div>
                  </Show>
                );
              }}
            </For>
          </div>
        </Show>
        <Show when={!overview() && games().length > 0}>
          {/* Keyed by tab/search so a new list slides in. */}
          <Show when={`${library.platform()}|${library.query()}|${library.filterKey()}`} keyed>
            <div
              class="tv-track"
              style={{ transform: `translate3d(${-(offsets()[focused()] ?? 0)}px, 0, 0)`, height: `${shelfH()}px` }}
            >
              <For each={windowed()}>
                {(i) => {
                  const g = () => games()[i];
                  return (
                    <Show when={g()}>
                      <div
                        class="tv-card"
                        classList={{ focused: i === focused(), "has-art": g()!.art > 0 }}
                        style={{
                          width: `${shelfH() / aspect(g()!)}px`,
                          height: `${shelfH()}px`,
                          transform: `translate3d(${offsets()[i]}px, 0, 0)`,
                        }}
                        onClick={() => (i === focused() ? setInfoId(g()!.id) : setFocused(i))}
                      >
                        {/* The zoom lives on the inner face: scaling the outer card would scale its translate too. */}
                        <div class="tv-card-face">
                          <Show when={g()!.art > 0} fallback={<span class="tv-card-title">{g()!.title}</span>}>
                            <img src={artUrl(g()!.id, g()!.art)} alt="" loading="lazy" decoding="async" draggable={false} />
                          </Show>
                        </div>
                      </div>
                    </Show>
                  );
                }}
              </For>
            </div>
          </Show>
        </Show>
      </div>

      <footer class="tv-hints">
        <Hint action="select" label={overview() ? "Open" : "Details"} />
        <Hint action="secondary" label="Search" />
        <Show when={!overview()}>
          <Hint action="favorite" label="Favorite" />
        </Show>
        <Hint actions={["prevPlatform", "nextPlatform"]} label="Platform" />
        <Hint action="menu" label="Menu" />
        {/* Clickable too, for the mouse. */}
        <button class="hint-btn" onClick={props.onExit} title="Back to the desktop view">
          <Hint action="toggleTv" label="Desktop mode" />
        </button>
        <span class="tv-count">
          {count() > 0 ? `${(overview() ? colFocused() : focused()) + 1} / ${count()}` : ""}
        </span>
      </footer>

      <Show when={menuOpen()}>
        <TvMenu
          onClose={() => setMenuOpen(false)}
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
      <Show when={infoId()} keyed>
        {(id) => <GameInfo id={id} onClose={() => setInfoId(null)} />}
      </Show>
    </div>
  );
}

function TvMenu(props: { onClose: () => void; onSearch: () => void; onSettings: () => void; onDesktop: () => void }) {
  return (
    <div class="overlay tv-menu-overlay" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <NavScope class="tv-menu" modal onBack={props.onClose}>
        <button class="tv-menu-item" data-autofocus onClick={props.onClose}>
          Resume
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
      </NavScope>
    </div>
  );
}

/** A long description cut to its opening sentences (see summary.ts). */
function brief(text: string) {
  return lead(text)?.join(" ") ?? text;
}
