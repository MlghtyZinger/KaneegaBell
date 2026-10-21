import { launchAnimation, launchDone } from "../fx/LaunchFx";
import { createEffect, createResource, createSignal, For, on, onCleanup, onMount, Show } from "solid-js";
import { api, artUrl, backloggdUrl, errorText, raMediaUrl } from "../api";
import { library } from "../state/library";
import { collections } from "../state/collections";
import { mainSeries } from "../state/series";
import { formatLastPlayed, formatPlayTime } from "../state/session";
import { NavScope } from "./NavScope";
import { Hint } from "./Hints";
import { hue } from "./GameCard";
import { openCollection } from "./Collections";
import { lead, sentences } from "../state/summary";
import { formatEarned } from "./Achievements";
import { LaunchWith, playWarning } from "./EmulatorChoice";

/** How long the selection must rest before the panel loads another game. */
const SETTLE_MS = 100;

/**
 * The game details sidebar (right of the grid): the chosen game's background,
 * logo, facts and Play / Favorite / More…. It opens with focus inside (Play
 * ready) and closes with B / Esc; a click on another cover shows that game.
 * Only what KB already has is shown: no downloads, achievements from the cache.
 *
 * `docked` is the list view's panel: always on screen, wider, with the whole
 * cover under the background and a box per topic. It follows the list's focus
 * and only takes the controller's focus (`active`) after A / Enter.
 */
export function GameSidebar(props: {
  id: number;
  onClose: () => void;
  onMore: (id: number) => void;
  onSettings: () => void;
  docked?: boolean;
  active?: boolean;
}) {
  // The game shown follows `id` once it has settled for a moment.
  const [id, setId] = createSignal(props.id);
  let timer = 0;
  createEffect(
    on(
      () => props.id,
      (next) => {
        clearTimeout(timer);
        timer = window.setTimeout(() => setId(next), SETTLE_MS);
      },
      { defer: true },
    ),
  );
  onCleanup(() => clearTimeout(timer));

  const row = () => library.games().find((g) => g.id === id());
  const [detail] = createResource(id, (gid) => api.getGame(gid).catch(() => null));
  const [glance] = createResource(id, (gid) => api.achievementGlance(gid).catch(() => null));
  // "Launch with" only for platforms with more than one emulator.
  const [emus, { refetch: refetchEmus }] = createResource(id, (gid) => api.gameEmulators(gid).catch(() => null));
  async function setEmulator(emulator: string | null) {
    try {
      await api.setGameEmulator(id(), emulator);
    } catch (e) {
      setLaunchError(errorText(e));
    }
    void refetchEmus();
  }
  const otherEmulators = () => {
    const e = emus();
    return e && e.options.length > 1 ? e : null;
  };
  const platform = () => {
    const pid = row()?.platformId;
    return library.platforms().find((p) => p.id === pid)?.name ?? pid?.toUpperCase() ?? "";
  };
  const meta = () => detail()?.meta;
  const year = () => {
    const d = meta()?.releaseDate;
    return d ? new Date(d * 1000).getUTCFullYear() : null;
  };
  const series = () => {
    const m = meta();
    return m ? mainSeries(m) : null;
  };
  const inCollections = () => collections.visible().filter((c) => c.games.includes(id()));
  /** The line above the title: its series, else the first collection it is in; a click opens it. */
  const tag = () => series() ?? inCollections()[0] ?? null;
  const open = (url: string) => void api.openUrl(url).catch(() => {});

  const [launching, setLaunching] = createSignal(false);
  const [launchError, setLaunchError] = createSignal<string | null>(null);
  createEffect(on(id, () => setLaunchError(null), { defer: true }));

  async function play() {
    if (launching()) return;
    setLaunching(true);
    setLaunchError(null);
    try {
      // The case opens (or the cartridge goes in) first; Back there cancels.
      if (await launchAnimation(id())) {
        await api.launchGame(id());
        props.onClose(); // the Now Playing screen takes over
      }
    } catch (e) {
      launchDone();
      setLaunchError(errorText(e));
    } finally {
      setLaunching(false);
    }
  }

  async function toggleFavorite() {
    const g = row();
    if (!g) return;
    await api.setFavorite(g.id, !g.favorite);
    await library.reload();
  }

  /** Play, Favorite, More…, the emulator pick and any warning: the same in both layouts. */
  const actions = (g: () => { favorite: boolean; id: number; playSeconds: number; lastPlayed: number | null }) => (
    <>
      <div class="side-actions">
        <button class="btn primary big" data-autofocus disabled={launching()} onClick={() => void play()}>
          {launching() ? "Starting…" : "▶ Play"}
        </button>
        <button class="btn big" classList={{ "fav-on": g().favorite }} onClick={() => void toggleFavorite()}>
          {g().favorite ? "★ Favorite" : "☆ Favorite"}
        </button>
        <button class="btn big" onClick={() => props.onMore(g().id)}>
          More…
        </button>
      </div>
      <Show when={otherEmulators()}>
        {(e) => (
          <div class="side-play-with">
            <span class="dim">Launch with</span>
            <LaunchWith emus={e()} onChange={(v) => void setEmulator(v)} />
          </div>
        )}
      </Show>
      <Show when={launchError()}>
        <p class="warn">{launchError()}</p>
      </Show>
      <Show when={!launchError() && playWarning(emus())}>
        <p class="warn small">{playWarning(emus())}</p>
      </Show>
      <p class="dim side-played">
        {g().playSeconds > 0
          ? `${formatPlayTime(g().playSeconds)} played · Last played ${formatLastPlayed(g().lastPlayed)}`
          : "Never played"}
      </p>
    </>
  );

  const content = () => (
    <Show when={row()} fallback={<p class="dim side-pad">Loading…</p>}>
      {(g) => (
        <>
          <Show when={g().hero || g().art > 0}>
            <div class="side-hero">
              <Show
                when={g().hero}
                fallback={
                  <Show when={g().art > 0}>
                    <img class="side-hero-cover" src={artUrl(g().id, g().art, "f")} alt="" decoding="async" />
                  </Show>
                }
              >
                {(v) => <SideHero src={artUrl(g().id, v(), "h")} />}
              </Show>
              <Show when={!props.docked && g().logo}>
                {(v) => <img class="side-logo" src={artUrl(g().id, v(), "l")} alt={g().title} decoding="async" />}
              </Show>
            </div>
          </Show>
          <Show
            when={props.docked}
            fallback={
              <div class="side-body">
                <div class="side-heading">
                  <Show
                    when={(g().icon ?? 0) > 0}
                    fallback={<div class="side-icon monogram" style={{ "--card-hue": hue(g().title) }}>{g().title.slice(0, 1).toUpperCase()}</div>}
                  >
                    <img class="side-icon" src={artUrl(g().id, g().icon!, "i")} alt="" />
                  </Show>
                  <div class="side-heading-copy">
                    <Show when={tag()}>
                      {(s) => (
                        <button class="side-series" title="Open this collection" onClick={() => openCollection(s().key)}>
                          {s().name}
                        </button>
                      )}
                    </Show>
                    <h2 class="side-title">{g().title}</h2>
                  </div>
                </div>
                <p class="side-meta">{[platform(), year(), meta()?.developer].filter(Boolean).join(" · ")}</p>
                <Show when={(meta()?.genres.length ?? 0) > 0}>
                  <div class="genre-row">
                    <For each={meta()!.genres.slice(0, 4)}>{(name) => <span class="badge">{name}</span>}</For>
                  </div>
                </Show>
                {actions(g)}
                <Show when={glance()}>
                  {(a) => (
                    <div class="side-ach" title="RetroAchievements (as last fetched)">
                      <div class="side-ach-line">
                        <span>
                          {a().earned} / {a().total} achievements
                        </span>
                        <span class="dim">
                          {a().pointsEarned} / {a().pointsTotal} points
                        </span>
                      </div>
                      <div class="side-ach-bar">
                        <div style={{ width: `${Math.round((a().earned / a().total) * 100)}%` }} />
                      </div>
                    </div>
                  )}
                </Show>
                <Show when={meta()?.description}>
                  <Clipped text={meta()!.description!} />
                </Show>
              </div>
            }
          >
            <div class="dock-head">
              <Show when={g().art > 0}>
                <img class="dock-cover" src={artUrl(g().id, g().art, "f")} alt="" decoding="async" />
              </Show>
              <div class="dock-title">
                <Show when={tag()}>
                      {(s) => (
                        <button class="side-series" title="Open this collection" onClick={() => openCollection(s().key)}>
                          {s().name}
                        </button>
                      )}
                    </Show>
                <h2 class="side-title">{g().title}</h2>
                <p class="side-meta">{[platform(), year(), meta()?.developer].filter(Boolean).join(" · ")}</p>
                <Show when={(meta()?.genres.length ?? 0) > 0}>
                  <div class="genre-row">
                    <For each={meta()!.genres.slice(0, 4)}>{(name) => <span class="badge">{name}</span>}</For>
                  </div>
                </Show>
              </div>
            </div>
            <div class="dock-boxes">
              <section class="dock-box">{actions(g)}</section>
              <Show when={glance()}>
                {(a) => (
                  <button class="dock-box dock-ach" title="Opens the details screen, which has every achievement" onClick={() => props.onMore(id())}>
                    <div class="dock-label">Achievements</div>
                    <div class="side-ach-line">
                      <span>{a().earned} / {a().total} unlocked</span>
                      <span class="dim">{a().pointsEarned} / {a().pointsTotal} points</span>
                    </div>
                    <div class="side-ach-bar">
                      <div style={{ width: `${Math.round((a().earned / a().total) * 100)}%` }} />
                    </div>
                    <For each={a().shown}>
                      {(x) => (
                        <div class="dock-ach-row" classList={{ locked: !x.earned }}>
                          <img src={raMediaUrl(x.badge)} alt="" loading="lazy" decoding="async" />
                          <span class="dock-ach-title">{x.title}</span>
                          <span class="dim">{x.earned ? formatEarned(x.earned) : `${x.points} pts`}</span>
                        </div>
                      )}
                    </For>
                  </button>
                )}
              </Show>
              <section class="dock-box dock-inline">
                <div class="dock-label">Links</div>
                <div class="genre-row">
                  <button class="link-btn" title="Opens in your browser" onClick={() => open(backloggdUrl(meta()?.title ?? g().title, meta()?.slug))}>
                    {meta()?.slug ? "Backloggd" : "Find on Backloggd"} <span aria-hidden="true">↗</span>
                  </button>
                  <Show when={meta()?.slug}>
                    {(slug) => (
                      <button class="link-btn" title="Opens in your browser" onClick={() => open(`https://www.igdb.com/games/${encodeURIComponent(slug())}`)}>
                        IGDB <span aria-hidden="true">↗</span>
                      </button>
                    )}
                  </Show>
                  <Show when={glance()?.raGameId}>
                    {(rid) => (
                      <button class="link-btn" title="Opens in your browser" onClick={() => open(`https://retroachievements.org/game/${rid()}`)}>
                        RetroAchievements <span aria-hidden="true">↗</span>
                      </button>
                    )}
                  </Show>
                </div>
              </section>
              <Show when={meta()?.description}>
                <section class="dock-box dock-about">
                  <div class="dock-label">About</div>
                  <Clipped text={meta()!.description!} />
                </section>
              </Show>
            </div>
          </Show>
          <Show when={!props.docked || props.active}>
            <div class="side-hints">
              <Hint action="select" label="Select" />
              <Hint action="favorite" label="Details" />
              <Hint action="back" label="Close" />
            </div>
          </Show>
        </>
      )}
    </Show>
  );

  return (
    <Show when={!props.docked || props.active} fallback={<div class="game-side docked">{content()}</div>}>
      <NavScope class={props.docked ? "game-side docked active" : "game-side"} modal onBack={props.onClose} onFavorite={() => props.onMore(id())} onMenu={props.onSettings}>
        {content()}
      </NavScope>
    </Show>
  );
}

/**
 * A background in the sidebar's 16:9 box. A 16:9 one fills it; a wide one
 * (SteamGridDB's are about 3:1) shows whole, with a blurred copy of itself
 * filling the bands above and below, instead of losing its sides.
 */
function SideHero(props: { src: string }) {
  const [wide, setWide] = createSignal(false);
  return (
    <>
      <Show when={wide()}>
        <img class="side-hero-blur" src={props.src} alt="" />
      </Show>
      <img
        class="side-hero-img"
        classList={{ wide: wide() }}
        src={props.src}
        alt=""
        decoding="async"
        onLoad={(e) => setWide(e.currentTarget.naturalWidth > e.currentTarget.naturalHeight * 2)}
      />
    </>
  );
}

/**
 * The description: in full when it fits the room it has, else as many of its first
 * whole sentences as fit (the full text is the tooltip). The panel is re-measured
 * whenever it changes size.
 */
function Clipped(props: { text: string }) {
  let el!: HTMLParagraphElement;
  const [cut, setCut] = createSignal(false);
  const fit = () => {
    el.style.height = "";
    el.style.flex = "";
    const brief = lead(props.text);
    el.textContent = brief ? brief.join(" ") : props.text;
    el.title = brief ? props.text : "";
    const room = el.clientHeight;
    setCut(false);
    if (el.scrollHeight <= room + 1) return;
    el.title = props.text;
    el.style.height = `${room}px`; // keep the room while trying shorter texts
    el.style.flex = "none";
    const all = brief ?? sentences(props.text);
    let lo = 1;
    let hi = all.length;
    while (lo < hi) {
      const mid = Math.ceil((lo + hi) / 2);
      el.textContent = all.slice(0, mid).join(" ");
      if (el.scrollHeight <= room + 1) lo = mid;
      else hi = mid - 1;
    }
    el.textContent = all.slice(0, lo).join(" ");
    setCut(el.scrollHeight > room + 1); // even the first sentence is too long: it fades
  };
  onMount(() => {
    const panel = el.closest(".game-side") ?? el;
    const ro = new ResizeObserver(fit);
    ro.observe(panel);
    onCleanup(() => ro.disconnect());
  });
  createEffect(() => {
    props.text;
    queueMicrotask(fit);
  });
  return <p ref={el} class="side-desc" classList={{ cut: cut() }} />;
}
