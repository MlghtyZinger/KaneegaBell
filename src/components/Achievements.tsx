import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { api, errorText, events, raMediaUrl, type Achievement, type GameAchievements } from "../api";

/** RA times ("YYYY-MM-DD HH:MM:SS", UTC) as a local date. */
function formatEarned(s: string): string {
  const d = new Date(s.replace(" ", "T") + "Z");
  return Number.isNaN(d.getTime()) ? s : d.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

function rarityText(a: Achievement): string {
  if (a.rarity === null) return "";
  const pct = a.rarity * 100;
  return `${pct < 1 ? pct.toFixed(1) : Math.round(pct)}% of players`;
}

/**
 * The game's RetroAchievements: progress and the full list, unlocked ones in
 * color. Renders nothing for consoles RA doesn't cover or when RA isn't set up.
 */
export function Achievements(props: { gameId: number }) {
  const [data, setData] = createSignal<GameAchievements | null>(null);
  const [error, setError] = createSignal<string | null>(null);
  const [loading, setLoading] = createSignal(false);

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
  // Connected to RA (or a pass finished) while this screen is open.
  const unlisten = events.onRaFinished(() => data()?.status !== "ok" && void load(false));
  onCleanup(() => void unlisten.then((f) => f()));

  const progress = () => data()?.progress ?? null;
  const pct = () => {
    const p = progress();
    return p && p.total > 0 ? Math.round((p.earned / p.total) * 100) : 0;
  };

  return (
    <Show when={error() || (data() && data()!.status !== "unsupported")}>
      <section class="ach">
        <div class="ach-head">
          <h3>Achievements</h3>
          <Show when={progress()}>
            {(p) => (
              <span class="dim">
                {p().earned} / {p().total} unlocked · {p().pointsEarned} / {p().pointsTotal} points
                {p().earnedHardcore > 0 ? ` · ${p().earnedHardcore} hardcore` : ""}
              </span>
            )}
          </Show>
          <Show when={data()?.status === "ok" || error()}>
            <button class="btn small" disabled={loading()} onClick={() => void load(true)}>
              {loading() ? "Checking…" : "Refresh"}
            </button>
          </Show>
        </div>
        <Show when={error()}>
          <p class="warn">Couldn't load achievements: {error()}</p>
        </Show>
        <Show when={data()?.stale && data()?.fetchedAt}>
          <p class="dim small">
            Offline: showing what RetroAchievements reported on {new Date(data()!.fetchedAt! * 1000).toLocaleString()}.
          </p>
        </Show>
        <Show when={data()?.status === "unknown"}>
          <p class="dim">
            RetroAchievements doesn't recognize this version of the game. Hacks, translations and some revisions have
            no achievements, or need a different dump.
            <span class="mono small ach-hash"> RA hash {data()!.hash}</span>
          </p>
        </Show>
        <Show when={data()?.status === "nohash"}>
          <p class="dim">This game's files can't be identified for RetroAchievements (format not supported).</p>
        </Show>
        <Show when={progress()}>
          {(p) => (
            <Show when={p().total > 0} fallback={<p class="dim">This game has no achievements yet.</p>}>
              <div class="ach-bar" role="progressbar" aria-valuenow={pct()} aria-valuemin={0} aria-valuemax={100}>
                <div style={{ width: `${pct()}%` }} />
              </div>
              <ul class="ach-list">
                <For each={p().achievements}>
                  {(a) => (
                    <li class="ach-item" classList={{ earned: !!a.earned }} tabindex="0">
                      <img
                        src={raMediaUrl(a.badge)}
                        alt=""
                        width="64"
                        height="64"
                        loading="lazy"
                        decoding="async"
                        // Offline and not cached yet: an empty tile, not a broken-image icon.
                        onError={(e) => (e.currentTarget.style.visibility = "hidden")}
                      />
                      <div class="ach-text">
                        <div class="ach-title">
                          {a.title}
                          <span class="ach-points">{a.points}</span>
                          <Show when={a.kind === "missable"}>
                            <span class="badge ach-kind">Missable</span>
                          </Show>
                        </div>
                        <div class="ach-desc">{a.description}</div>
                        <div class="dim small">
                          {a.earned
                            ? `Unlocked ${formatEarned(a.earned)}${a.hardcore ? " (hardcore)" : ""}`
                            : rarityText(a)}
                        </div>
                      </div>
                    </li>
                  )}
                </For>
              </ul>
            </Show>
          )}
        </Show>
      </section>
    </Show>
  );
}
