import { createResource, createSignal, For, Show } from "solid-js";
import { api, errorText } from "../api";
import { formatLastPlayed, formatPlayTime } from "../state/session";
import { library } from "../state/library";
import { NavScope } from "../components/NavScope";
import { Hint } from "../components/Hints";

function formatSize(bytes: number): string {
  if (bytes >= 1 << 30) return `${(bytes / (1 << 30)).toFixed(1)} GB`;
  if (bytes >= 1 << 20) return `${(bytes / (1 << 20)).toFixed(0)} MB`;
  return `${Math.max(1, Math.round(bytes / 1024))} KB`;
}

/** Quote an argv element the way a Windows command line would show it. */
function showArg(a: string): string {
  return /[\s"]/.test(a) || a === "" ? `"${a.replace(/"/g, '\\"')}"` : a;
}

/**
 * Game details with a Play button: what the scanner found, play time, and the
 * exact command a launch runs. (Milestone d turns this into the full detail
 * view with art and metadata.)
 */
export function GameInfo(props: { id: number; onClose: () => void }) {
  const [game] = createResource(() => props.id, api.getGame);
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
            <>
              <h2>{g().title}</h2>
              <Show when={g().variant}>
                <p class="dim">{g().variant}</p>
              </Show>
              <div class="row">
                <button class="btn primary big" data-autofocus disabled={launching()} onClick={() => void play()}>
                  {launching() ? "Starting…" : "▶ Play"}
                </button>
                <button class="btn big" classList={{ "fav-on": isFavorite() }} onClick={() => void toggleFavorite()}>
                  {isFavorite() ? "★ Favorite" : "☆ Favorite"}
                </button>
                <span class="dim">
                  {formatPlayTime(g().playSeconds)} played · Last played {formatLastPlayed(g().lastPlayed)}
                </span>
              </div>
              <Show when={launchError()}>
                <p class="warn">{launchError()}</p>
              </Show>
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
            </>
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
    </div>
  );
}
