import { createMemo, createSignal, For, onMount, Show } from "solid-js";
import { Portal } from "solid-js/web";
import { api, errorText, raMediaUrl, type Achievement, type GameAchievements } from "../api";
import { NavScope } from "./NavScope";
import { Hint } from "./Hints";
import { formatEarned } from "./Achievements";

type Filter = "all" | "unlocked" | "locked" | "missable";
const FILTERS: Filter[] = ["all", "unlocked", "locked", "missable"];
const FILTER_LABEL: Record<Filter, string> = { all: "All", unlocked: "Unlocked", locked: "Locked", missable: "Missable" };
const SORTS: { label: string; by: (a: Achievement, b: Achievement) => number }[] = [
  { label: "Points, most first", by: (a, b) => b.points - a.points || a.id - b.id },
  { label: "Rarest first", by: (a, b) => (a.rarity ?? 1) - (b.rarity ?? 1) || a.id - b.id },
  { label: "Unlock date, newest", by: (a, b) => (b.earned ?? "").localeCompare(a.earned ?? "") || a.id - b.id },
  { label: "RetroAchievements' order", by: (a, b) => a.id - b.id },
];

function rarityText(a: Achievement): string {
  if (a.rarity === null) return "";
  const pct = a.rarity * 100;
  return `${pct < 1 ? pct.toFixed(1) : Math.round(pct)}% of players`;
}

/** Why a game has no list to show (mirrors the details screen). */
function reason(d: GameAchievements): string {
  if (d.status === "unknown") return `RetroAchievements has achievements for ${d.raTitle ?? "this game"}, but not for this version of it.`;
  if (d.status === "noset") return "No achievements found for this game on RetroAchievements.";
  if (d.status === "nohash") return "This game's files can't be identified for RetroAchievements.";
  return "Achievements aren't available yet. Refresh when you're online.";
}

/**
 * The full RetroAchievements window: progress, filters, sort, a list with the
 * focused achievement's details beside it. Works with a controller alone:
 * D-pad moves, B closes, LB/RB filter, X sorts, Y jumps to the next achievement
 * worth the most, Menu refreshes. Reads the cache; only Refresh goes online.
 */
export function AchievementsWindow(props: { gameId: number; gameTitle?: string; tv: boolean; onClose: () => void }) {
  const [data, setData] = createSignal<GameAchievements | null>(null);
  const [error, setError] = createSignal<string | null>(null);
  const [loading, setLoading] = createSignal(false);
  const [filter, setFilter] = createSignal<Filter>("all");
  const [sort, setSort] = createSignal(0);
  const [sel, setSel] = createSignal<number | null>(null);

  async function load(refresh: boolean) {
    setLoading(true);
    try {
      setData(await api.getAchievements(props.gameId, refresh));
      setError(null);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setLoading(false);
    }
  }
  onMount(() => void load(false));

  const progress = () => data()?.progress ?? null;
  const counts = createMemo(() => {
    const all = progress()?.achievements ?? [];
    return {
      all: all.length,
      unlocked: all.filter((a) => a.earned).length,
      locked: all.filter((a) => !a.earned).length,
      missable: all.filter((a) => a.kind === "missable").length,
    };
  });
  const list = createMemo(() => {
    const all = progress()?.achievements ?? [];
    const keep: Record<Filter, (a: Achievement) => boolean> = {
      all: () => true,
      unlocked: (a) => !!a.earned,
      locked: (a) => !a.earned,
      missable: (a) => a.kind === "missable",
    };
    return all.filter(keep[filter()]).sort(SORTS[sort()].by);
  });
  // The panel follows the focused row; with nothing chosen it shows the first.
  const focused = createMemo(() => list().find((a) => a.id === sel()) ?? list()[0] ?? null);

  const cycleFilter = (delta: 1 | -1) => {
    const i = FILTERS.indexOf(filter());
    setFilter(FILTERS[(i + delta + FILTERS.length) % FILTERS.length]);
  };
  const nextWorth = () => {
    const best = (progress()?.achievements ?? []).filter((a) => !a.earned).sort((a, b) => b.points - a.points || a.id - b.id)[0];
    if (!best) return;
    setFilter("all");
    setSel(best.id);
  };
  const pct = () => {
    const p = progress();
    return p && p.total > 0 ? Math.round((p.earned / p.total) * 100) : 0;
  };

  // A portal: the details panel it opens from is transformed, which would trap this fixed overlay inside it.
  return (
    <Portal>
    <div class="overlay" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <NavScope
        class={`panel ach-win${props.tv ? " ach-win-tv" : ""}`}
        modal
        onBack={props.onClose}
        onTab={cycleFilter}
        onSecondary={() => setSort((sort() + 1) % SORTS.length)}
        onFavorite={nextWorth}
        onMenu={() => void load(true)}
      >
        <div class="ach-win-head">
          <h2>{props.gameTitle ?? "Achievements"}</h2>
          <Show when={progress()}>
            {(p) => (
              <span class="dim">
                {p().earned} / {p().total} unlocked · {p().pointsEarned} / {p().pointsTotal} points
                {p().earnedHardcore > 0 ? ` · ${p().earnedHardcore} hardcore` : ""}
              </span>
            )}
          </Show>
          <div class="ach-win-spacer" />
          <button class="btn small" disabled={loading()} onClick={() => void load(true)}>
            {loading() ? "Checking…" : "Refresh"}
          </button>
        </div>
        <Show when={progress()}>
          <div class="ach-bar" role="progressbar" aria-valuenow={pct()} aria-valuemin={0} aria-valuemax={100}>
            <div style={{ width: `${pct()}%` }} />
          </div>
        </Show>
        <Show when={error()}>
          <p class="warn">Couldn't load achievements: {error()}</p>
        </Show>
        <Show when={data() && !progress()}>
          <p class="dim">{reason(data()!)}</p>
        </Show>
        <Show when={progress() && progress()!.total > 0}>
          <div class="ach-win-chips">
            <For each={FILTERS}>
              {(f) => (
                <button class="btn small" aria-pressed={filter() === f} classList={{ primary: filter() === f }} onClick={() => setFilter(f)}>
                  {FILTER_LABEL[f]} {counts()[f]}
                </button>
              )}
            </For>
            <div class="ach-win-spacer" />
            <button class="btn small" onClick={() => setSort((sort() + 1) % SORTS.length)}>
              {SORTS[sort()].label} ▾
            </button>
          </div>
          <div class="ach-win-body">
            <div class="ach-win-list">
              <For each={list()} fallback={<p class="dim">Nothing in this filter.</p>}>
                {(a) => (
                  <button class="ach-win-row" classList={{ earned: !!a.earned }} onFocus={() => setSel(a.id)} onMouseEnter={() => setSel(a.id)}>
                    <img
                      src={raMediaUrl(a.badge)}
                      alt=""
                      width="56"
                      height="56"
                      loading="lazy"
                      decoding="async"
                      onError={(e) => (e.currentTarget.style.visibility = "hidden")}
                    />
                    <span class="ach-win-row-text">
                      <span class="ach-title">
                        {a.title}
                        <span class="ach-points">{a.points}</span>
                        <Show when={a.kind === "missable"}>
                          <span class="badge ach-kind">Missable</span>
                        </Show>
                      </span>
                      <span class="dim small">{a.earned ? `Unlocked ${formatEarned(a.earned)}${a.hardcore ? " (hardcore)" : ""}` : rarityText(a)}</span>
                    </span>
                  </button>
                )}
              </For>
            </div>
            <Show when={focused()}>
              {(a) => (
                <div class="ach-win-panel" aria-live="polite">
                  <img src={raMediaUrl(a().badge)} alt="" width="128" height="128" onError={(e) => (e.currentTarget.style.visibility = "hidden")} />
                  <div class="ach-title">{a().title}</div>
                  <div class="ach-desc">{a().description}</div>
                  <div>
                    <span class="ach-points">{a().points} pts</span>
                    <Show when={a().kind === "missable"}>
                      <span class="badge ach-kind">Missable</span>
                    </Show>
                  </div>
                  <div class="dim">
                    {a().earned ? `Unlocked ${formatEarned(a().earned!)}${a().hardcore ? " (hardcore)" : ""}` : "Locked"}
                  </div>
                  <Show when={a().rarity !== null}>
                    <div class="dim small">{rarityText(a())}</div>
                  </Show>
                </div>
              )}
            </Show>
          </div>
        </Show>
        <div class="ach-win-hints">
          <Hint action="prevPlatform" label="Filter" />
          <Hint action="secondary" label="Sort" />
          <Hint action="favorite" label="Next worth the most" />
          <Hint action="menu" label="Refresh" />
          <Hint action="back" label="Close" />
        </div>
      </NavScope>
    </div>
    </Portal>
  );
}
