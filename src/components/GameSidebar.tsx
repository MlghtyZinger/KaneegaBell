import { createEffect, createResource, createSignal, For, on, onCleanup, Show } from "solid-js";
import { api, artUrl, errorText } from "../api";
import { library } from "../state/library";
import { mainSeries } from "../state/series";
import { formatLastPlayed, formatPlayTime } from "../state/session";
import { NavScope } from "./NavScope";
import { Hint } from "./Hints";
import { PlayWith, playWarning } from "./EmulatorChoice";

/** How long the selection must rest before the panel loads another game. */
const SETTLE_MS = 100;

/**
 * The game details sidebar (right of the grid): the chosen game's background,
 * logo, facts and Play / Favorite / More…. It opens with focus inside (Play
 * ready) and closes with B / Esc; a click on another cover shows that game.
 * Only what KB already has is shown: no downloads, achievements from the cache.
 */
export function GameSidebar(props: { id: number; onClose: () => void; onMore: (id: number) => void; onSettings: () => void }) {
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
  // "Play with…" only for platforms with more than one emulator.
  const [emus] = createResource(id, (gid) => api.gameEmulators(gid).catch(() => null));
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

  const [launching, setLaunching] = createSignal(false);
  const [launchError, setLaunchError] = createSignal<string | null>(null);
  createEffect(on(id, () => setLaunchError(null), { defer: true }));

  async function play(emulator: string | null = null) {
    if (launching()) return;
    setLaunching(true);
    setLaunchError(null);
    try {
      await api.launchGame(id(), emulator);
      props.onClose(); // the Now Playing screen takes over
    } catch (e) {
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

  return (
    <NavScope class="game-side" modal onBack={props.onClose} onFavorite={() => props.onMore(id())} onMenu={props.onSettings}>
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
                {(v) => <img class="side-hero-img" src={artUrl(g().id, v(), "h")} alt="" decoding="async" />}
              </Show>
              <Show when={g().logo}>
                {(v) => <img class="side-logo" src={artUrl(g().id, v(), "l")} alt={g().title} decoding="async" />}
              </Show>
            </div>
            </Show>
            <div class="side-body">
              <Show when={series()}>
                {(s) => <div class="side-series">{s().name}</div>}
              </Show>
              <h2 class="side-title">{g().title}</h2>
              <p class="side-meta">{[platform(), year(), meta()?.developer].filter(Boolean).join(" · ")}</p>
              <Show when={(meta()?.genres.length ?? 0) > 0}>
                <div class="genre-row">
                  <For each={meta()!.genres.slice(0, 4)}>{(name) => <span class="badge">{name}</span>}</For>
                </div>
              </Show>
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
                    <PlayWith emus={e()} onPick={(v) => void play(v)} />
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
                <p class="side-desc">{meta()!.description}</p>
              </Show>
            </div>
            <div class="side-hints">
              <Hint action="select" label="Select" />
              <Hint action="favorite" label="Details" />
              <Hint action="back" label="Close" />
            </div>
          </>
        )}
      </Show>
    </NavScope>
  );
}
