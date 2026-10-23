import { createMemo, createSignal, For, onMount, Show } from "solid-js";
import { Portal } from "solid-js/web";
import { api, errorText, raMediaUrl, type Achievement, type GameAchievements } from "../api";
import { NavScope } from "./NavScope";
import { Hint } from "./Hints";
import { Picker } from "./Picker";
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
const SORT_OPTIONS = SORTS.map((s, i) => ({ value: String(i), label: s.label }));
const KIND_LABEL: Record<string, string> = { progression: "Progression", win_condition: "Win condition", missable: "Missable" };

function rarityPct(a: Achievement): string {
  if (a.rarity === null) return "";
  const pct = a.rarity * 100;
  return `${pct < 1 ? pct.toFixed(1) : Math.round(pct)}%`;
}

/** Why a game has no list to show (mirrors the details screen). */
function reason(d: GameAchievements): string {
  if (d.status === "unknown") return `RetroAchievements has achievements for ${d.raTitle ?? "this game"}, but not for this version of it.`;
  if (d.status === "noset") return "No achievements found for this game on RetroAchievements.";
  if (d.status === "nohash") return "This game's files can't be identified for RetroAchievements.";
  return "Achievements aren't available yet. Refresh when you're online.";
}

function Badge(props: { a: Achievement; big?: boolean }) {
  return (
    <span class="ach-win-badge" classList={{ on: !!props.a.earned, big: props.big }}>
      <img src={raMediaUrl(props.a.badge)} alt="" decoding="async" onError={(e) => (e.currentTarget.style.visibility = "hidden")} />
    </span>
  );
}

function Kind(props: { a: Achievement }) {
  return (
    <Show when={props.a.kind} fallback={<span />}>
      <span class="ach-win-tag" data-kind={props.a.kind}>{KIND_LABEL[props.a.kind!]}</span>
    </Show>
  );
}

/**
 * The full RetroAchievements window: progress, filters, sort, a list with the
 * focused achievement's details beside it. Works with a controller alone:
 * D-pad moves, B closes, LB/RB filter, X opens the sort list, Menu refreshes.
 * Reads the cache; only Refresh goes online.
 */
export function AchievementsWindow(props: { gameId: number; tv: boolean; onClose: () => void }) {
  const [data, setData] = createSignal<GameAchievements | null>(null);
  const [error, setError] = createSignal<string | null>(null);
  const [loading, setLoading] = createSignal(false);
  const [filter, setFilter] = createSignal<Filter>("all");
  const [sort, setSort] = createSignal(0);
  const [sortOpen, setSortOpen] = createSignal(false);
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
  const pct = () => {
    const p = progress();
    return p && p.total > 0 ? Math.round((p.earned / p.total) * 100) : 0;
  };
  const openRa = () => void api.openUrl(`https://retroachievements.org/game/${data()!.raGameId}`).catch(() => {});

  // A portal: the details panel it opens from is transformed, which would trap this fixed overlay inside it.
  return (
    <Portal>
      <div class="overlay" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
        <NavScope
          class={`panel ach-win${props.tv ? " ach-win-tv" : ""}`}
          modal
          onBack={props.onClose}
          onTab={cycleFilter}
          onSecondary={() => setSortOpen(true)}
          onMenu={() => void load(true)}
        >
          <div class="ach-win-top">
            <h2>Achievements</h2>
            <Show when={progress()}>{(p) => <span class="dim">{p().title}</span>}</Show>
            <div class="ach-win-spacer" />
            <button class="btn small" disabled={loading()} onClick={() => void load(true)}>
              {loading() ? "Checking…" : "Refresh"}
            </button>
            <Show when={data()?.raGameId}>
              <button class="btn small" onClick={openRa}>Open on RetroAchievements ↗</button>
            </Show>
          </div>
          <Show when={error()}>
            <p class="warn">Couldn't load achievements: {error()}</p>
          </Show>
          <Show when={data() && !progress()}>
            <p class="dim">{reason(data()!)}</p>
          </Show>
          <Show when={progress() && progress()!.total > 0}>
            <div class="ach-win-progress">
              <Show when={progress()!.icon}>
                <img class="ach-win-icon" src={raMediaUrl(progress()!.icon!, "rai")} alt="" onError={(e) => (e.currentTarget.style.visibility = "hidden")} />
              </Show>
              <div class="ach-win-progress-main">
                <div class="ach-win-count">
                  <b>{progress()!.earned}</b>
                  <span class="dim">/ {progress()!.total} achievements unlocked</span>
                  <div class="ach-win-spacer" />
                  <span class="ach-win-gold">{progress()!.pointsEarned}</span>
                  <span class="dim">/ {progress()!.pointsTotal} points</span>
                  <Show when={progress()!.earnedHardcore > 0}>
                    <span>{progress()!.earnedHardcore} in hardcore</span>
                  </Show>
                </div>
                <div class="ach-bar" role="progressbar" aria-valuenow={pct()} aria-valuemin={0} aria-valuemax={100}>
                  <div style={{ width: `${pct()}%` }} />
                </div>
              </div>
            </div>
            <div class="ach-win-chips">
              <For each={FILTERS}>
                {(f) => (
                  <button class="btn small ach-chip" aria-pressed={filter() === f} classList={{ primary: filter() === f }} onClick={() => setFilter(f)}>
                    {FILTER_LABEL[f]} {counts()[f]}
                  </button>
                )}
              </For>
              <div class="ach-win-spacer" />
              <span class="dim">Sort by</span>
              <Picker
                title="Sort by"
                value={String(sort())}
                options={SORT_OPTIONS}
                onChange={(v) => setSort(Number(v))}
                open={sortOpen()}
                onOpenChange={setSortOpen}
              />
            </div>
            <div class="ach-win-body">
              <div class="ach-win-list">
                <For each={list()} fallback={<p class="dim">Nothing in this filter.</p>}>
                  {(a) => (
                    <button class="ach-win-row" classList={{ earned: !!a.earned }} onFocus={() => setSel(a.id)}>
                      <Badge a={a} />
                      <span class="ach-win-row-text">
                        <b>{a.title}</b>
                        <span class="dim">{a.description}</span>
                      </span>
                      <span class="ach-win-pill">{a.points} pts</span>
                      <Kind a={a} />
                      <span class="ach-win-date">
                        {a.earned ? formatEarned(a.earned) : "Locked"}
                        <Show when={a.hardcore}>
                          <i class="ach-hc">HC</i>
                        </Show>
                      </span>
                      <span class="ach-win-rar dim">{rarityPct(a)}</span>
                    </button>
                  )}
                </For>
              </div>
              <Show when={focused()}>
                {(a) => (
                  <div class="ach-win-panel">
                    <div class="ach-win-panel-head">
                      <Badge a={a()} big />
                      <div class="ach-win-panel-meta">
                        <div class="ach-win-panel-title">{a().title}</div>
                        <div class="ach-win-panel-tags">
                          <Kind a={a()} />
                          <span class="ach-win-pill">{a().points} pts</span>
                        </div>
                      </div>
                    </div>
                    <p class="ach-win-panel-desc">{a().description}</p>
                    <div class="ach-win-stats">
                      <div>
                        <small class="dim">Unlocked</small>
                        <b>{a().earned ? formatEarned(a().earned!) : "Locked"}</b>
                      </div>
                      <div>
                        <small class="dim">Mode</small>
                        <b>{a().earned ? (a().hardcore ? "Hardcore" : "Softcore") : "Not yet"}</b>
                      </div>
                    </div>
                    <Show when={a().rarity !== null}>
                      <div>
                        <div class="ach-win-rarity-row">
                          <span class="dim">Rarity</span>
                          <span>{rarityPct(a())} of players</span>
                        </div>
                        <div class="ach-bar">
                          <div style={{ width: `${Math.max(a().rarity! * 100, 2)}%` }} />
                        </div>
                      </div>
                    </Show>
                  </div>
                )}
              </Show>
            </div>
          </Show>
          <div class="ach-win-hints">
            <Hint actions={["prevPlatform", "nextPlatform"]} label="Filter" />
            <Hint action="secondary" label="Sort" />
            <Hint action="back" label="Close" />
          </div>
        </NavScope>
      </div>
    </Portal>
  );
}
