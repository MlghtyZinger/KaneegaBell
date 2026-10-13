import { createResource, createSignal, For, onCleanup, Show } from "solid-js";
import { api, artUrl, backloggdUrl, errorText, events, type GameDetail } from "../api";
import { formatLastPlayed, formatPlayTime } from "../state/session";
import { library } from "../state/library";
import { NavScope } from "../components/NavScope";
import { CoverPicker, type PictureKind } from "../components/CoverPicker";
import { MatchPicker } from "../components/MatchPicker";
import { Hint } from "../components/Hints";
import { Achievements } from "../components/Achievements";

function formatSize(bytes: number): string {
  if (bytes >= 1 << 30) return `${(bytes / (1 << 30)).toFixed(1)} GB`;
  if (bytes >= 1 << 20) return `${(bytes / (1 << 20)).toFixed(0)} MB`;
  return `${Math.max(1, Math.round(bytes / 1024))} KB`;
}

/** "2006 · Clover Studio · Capcom": release year, developer, publisher (when known). */
function metaLine(g: GameDetail): string {
  const m = g.meta;
  const year = m.releaseDate ? new Date(m.releaseDate * 1000).getUTCFullYear() : null;
  const pub = m.publisher && m.publisher !== m.developer ? m.publisher : null;
  return [year, m.developer, pub].filter(Boolean).join(" · ");
}

/** Quote an argv element the way a Windows command line would show it. */
function showArg(a: string): string {
  return /[\s"]/.test(a) || a === "" ? `"${a.replace(/"/g, '\\"')}"` : a;
}

/**
 * Game details with a Play button: cover, play time, what the scanner found,
 * and the exact command a launch runs.
 */
export function GameInfo(props: { id: number; onClose: () => void }) {
  const [game, { refetch }] = createResource(() => props.id, api.getGame);
  const [refreshing, setRefreshing] = createSignal(false);
  const [picking, setPicking] = createSignal<PictureKind | null>(null);
  const [matching, setMatching] = createSignal(false);
  const [services] = createResource(() => api.getServices().catch(() => null));

  async function refreshCover() {
    setRefreshing(true);
    try {
      await api.refreshArt(props.id);
    } catch {
      setRefreshing(false);
    }
  }
  // The art job reports back when it's done; show the new cover then.
  const unlisten = events.onArtFinished(() => {
    setRefreshing(false);
    void refetch();
  });
  onCleanup(() => void unlisten.then((f) => f()));
  const [argv] = createResource(
    () => props.id,
    (id) => api.previewLaunch(id).then((a) => ({ ok: true as const, a }), (e) => ({ ok: false as const, e: errorText(e) })),
  );
  const [launching, setLaunching] = createSignal(false);
  const [launchError, setLaunchError] = createSignal<string | null>(null);
  const [favorite, setFavorite] = createSignal<boolean | null>(null);
  const isFavorite = () => favorite() ?? library.games().find((g) => g.id === props.id)?.favorite ?? false;

  async function toggleFavorite() {
    const next = !isFavorite();
    setFavorite(next);
    await api.setFavorite(props.id, next);
    await library.reload();
  }

  async function play() {
    if (launching()) return;
    setLaunching(true);
    setLaunchError(null);
    try {
      await api.launchGame(props.id);
      props.onClose(); // the Now Playing screen takes over
    } catch (e) {
      setLaunchError(errorText(e));
    } finally {
      setLaunching(false);
    }
  }

  return (
    <div class="overlay" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <NavScope class="panel info-panel" modal onBack={props.onClose} onFavorite={() => void toggleFavorite()}>
        <Show when={game()} fallback={<p>Loading…</p>}>
          {(g) => (
            <div class="info-layout">
              <div class="info-art">
                <Show
                  when={g().art > 0}
                  fallback={<div class="info-art-empty dim">No cover yet</div>}
                >
                  <img src={artUrl(g().id, g().art, "f")} alt={`${g().title} cover`} decoding="async" />
                </Show>
                <div class="row tight">
                  <button class="btn small" onClick={() => setPicking("cover")}>
                    Change cover…
                  </button>
                  <button class="btn small" disabled={refreshing()} onClick={() => void refreshCover()}>
                    {refreshing() ? "Looking…" : "Refresh"}
                  </button>
                </div>
                <Show when={services()?.sgdb}>
                  <div class="row tight">
                    <button class="btn small" onClick={() => setPicking("hero")}>
                      Background…
                    </button>
                    <button class="btn small" onClick={() => setPicking("logo")}>
                      Logo…
                    </button>
                  </div>
                </Show>
                <Show when={g().artSource}>
                  <span class="dim small">
                    Cover:{" "}
                    {({ libretro: "libretro-thumbnails", gametdb: "GameTDB", igdb: "IGDB", sgdb: "SteamGridDB", local: "the game's icon", file: "your image" } as Record<string, string>)[
                      g().artSource!
                    ] ?? g().artSource}
                  </span>
                </Show>
              </div>
              <div class="info-main">
              <h2>{g().title}</h2>
              <Show when={g().variant}>
                <p class="dim">{g().variant}</p>
              </Show>
              <Show when={metaLine(g())}>
                <p class="meta-line">{metaLine(g())}</p>
              </Show>
              <Show when={g().meta.genres.length > 0}>
                <div class="genre-row">
                  <For each={g().meta.genres}>{(name) => <span class="badge">{name}</span>}</For>
                </div>
              </Show>
              <div class="row">
                <button class="btn primary big" data-autofocus disabled={launching()} onClick={() => void play()}>
                  {launching() ? "Starting…" : "▶ Play"}
                </button>
                <button class="btn big" classList={{ "fav-on": isFavorite() }} onClick={() => void toggleFavorite()}>
                  {isFavorite() ? "★ Favorite" : "☆ Favorite"}
                </button>
                <span class="dim">
                  {g().playSeconds > 0
                    ? `${formatPlayTime(g().playSeconds)} played · Last played ${formatLastPlayed(g().lastPlayed)}`
                    : "Never played"}
                </span>
              </div>
              <Show when={launchError()}>
                <p class="warn">{launchError()}</p>
              </Show>
              <Show when={g().meta.description}>
                <p class="description">{g().meta.description}</p>
              </Show>
              <div class="row tight">
                <button
                  class="btn small"
                  title="Opens in your browser"
                  onClick={() => void api.openUrl(backloggdUrl(g().meta.title ?? g().title, g().meta.slug)).catch(() => {})}
                >
                  {g().meta.slug ? "Backloggd ↗" : "Find on Backloggd ↗"}
                </button>
              </div>
              <Show when={services()?.igdb}>
                <div class="row tight">
                  <button class="btn small" onClick={() => setMatching(true)}>
                    Fix match…
                  </button>
                  <span class="dim small">
                    {g().meta.igdbId
                      ? `IGDB: ${g().meta.title ?? "matched"}${g().meta.matchStatus === "manual" ? " (your pick)" : ""}`
                      : g().meta.matchStatus === "notfound"
                        ? "Not found on IGDB"
                        : "Not looked up on IGDB yet"}
                  </span>
                </div>
              </Show>
              <Achievements gameId={g().id} />
              <dl class="facts">
                <dt>Platform</dt>
                <dd>{g().platformId.toUpperCase()}</dd>
                <Show when={g().region}>
                  <dt>Region</dt>
                  <dd>{g().region}</dd>
                </Show>
                <Show when={g().serial}>
                  <dt>Serial</dt>
                  <dd>{g().serial}</dd>
                </Show>
                <dt>Size</dt>
                <dd>{formatSize(g().totalSize)}</dd>
                <dt>Launch file</dt>
                <dd class="mono">{g().launchPath}</dd>
                <Show when={g().files.length > 1}>
                  <dt>Discs</dt>
                  <dd>
                    <For each={g().files}>
                      {([path, disc]) => (
                        <div class="mono">
                          {disc ? `Disc ${disc}: ` : ""}
                          {path}
                        </div>
                      )}
                    </For>
                  </dd>
                </Show>
              </dl>
              <h3>Launch command</h3>
              <Show when={argv()}>
                {(r) => {
                  const res = r();
                  return res.ok ? <pre class="cmd">{res.a.map(showArg).join(" ")}</pre> : <p class="warn">{res.e}</p>;
                }}
              </Show>
              </div>
            </div>
          )}
        </Show>
        <div class="panel-actions">
          <div class="hints-row">
            <Hint action="select" label="Select" />
            <Hint action="favorite" label="Favorite" />
            <Hint action="back" label="Close" />
          </div>
          <button class="btn" onClick={props.onClose}>
            Close
          </button>
        </div>
      </NavScope>
      <Show when={matching() && game()}>
        {(g) => (
          <MatchPicker
            gameId={g().id}
            title={g().title}
            onClose={() => setMatching(false)}
            onChanged={() => {
              void refetch();
              void library.reload();
            }}
          />
        )}
      </Show>
      <Show when={picking() && game()}>
        {(g) => (
          <CoverPicker
            gameId={g().id}
            title={g().title}
            kind={picking()!}
            onClose={() => setPicking(null)}
            onChanged={() => {
              void refetch();
              void library.reload();
            }}
          />
        )}
      </Show>
    </div>
  );
}
