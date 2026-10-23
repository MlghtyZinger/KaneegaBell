import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { api, errorText, events, raMediaUrl, type Achievement, type GameAchievements } from "../api";
import { AchievementsWindow } from "./AchievementsWindow";
import { fx } from "../state/fx";

/** RA times ("YYYY-MM-DD HH:MM:SS", UTC) as a local date. */
export function formatEarned(s: string): string {
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
export function Achievements(props: { gameId: number; /** The game's RetroAchievements id when known (for a link). */ onRaGame?: (id: number | null) => void }) {
  const [data, setData] = createSignal<GameAchievements | null>(null);
  const [error, setError] = createSignal<string | null>(null);
  const [loading, setLoading] = createSignal(false);
  const [windowOpen, setWindowOpen] = createSignal(false);

  async function load(refresh: boolean) {
    setLoading(true);
    try {
      const d = await api.getAchievements(props.gameId, refresh);
      setData(d);
      props.onRaGame?.(d.raGameId ?? null);
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
          <Show when={data()?.status === "ok"}>
            <button class="btn small" onClick={() => setWindowOpen(true)}>Open window</button>
          </Show>
        </div>
        <Show when={windowOpen()}>
          <AchievementsWindow gameId={props.gameId} tv={fx.tv()} onClose={() => setWindowOpen(false)} />
        </Show>
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
            RetroAchievements has achievements for <b class="ach-ra-title">{data()!.raTitle ?? "this game"}</b>, but
            not for this version of it.
            <Show when={data()!.accepted.length > 0}> Versions it accepts:</Show>
          </p>
          <Show when={data()!.accepted.length > 0}>
            <ul class="ach-versions">
              <For each={[...data()!.accepted].sort((a, b) => Number(a.patched) - Number(b.patched))}>
                {(d) => (
                  <li tabindex="0">
                    <span class="mono">{d.name}</span>
                    <Show when={d.patched}>
                      <span class="badge ach-patch">RA patch</span>
                    </Show>
                  </li>
                )}
              </For>
            </ul>
            <Show when={data()!.accepted.some((d) => d.patched)}>
              <p class="dim small">
                "RA patch" versions are a regular dump with a patch from RetroAchievements applied (fixes or
                translations); the patch is listed on the game's RetroAchievements page (Links, under the cover).
              </p>
            </Show>
          </Show>
          <span class="mono small ach-hash">This file's RA hash: {data()!.hash}</span>
        </Show>
        <Show when={data()?.status === "noset"}>
          <p class="dim">
            No achievements found for this game on RetroAchievements (searched its list of this console's games with
            achievements by title).
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
