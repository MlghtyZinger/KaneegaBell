import { createResource, createSignal, Show } from "solid-js";
import { api, errorText } from "../api";
import { FileBrowser } from "./FileBrowser";

/** The picker changes process tracking only; Steam still launches the game. */
export function SteamTracking(props: { id: number }) {
  const [game, { refetch }] = createResource(() => props.id, (id) => api.steamGame(id));
  const [browsing, setBrowsing] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const choose = async (path: string | null) => {
    setBrowsing(false); setError(null);
    try { await api.setSteamTracking(props.id, path); await refetch(); }
    catch (e) { setError(errorText(e)); }
  };
  return <Show when={game()}>{(g) => <div class="steam-tracking">
    <p class="dim small">Launches through Steam · {g().trackingExe ? "Tracking your chosen executable" : g().executables.length ? "Tracking Steam's game executable" : "Choose the game executable to enable safe tracking"}</p>
    <button class="btn small" onClick={() => setBrowsing(true)}>Choose game executable…</button>
    <Show when={g().trackingExe}><button class="btn small" onClick={() => void choose(null)}>Use Steam's default</button></Show>
    <Show when={error()}><p class="warn">{error()}</p></Show>
    <Show when={browsing()}><FileBrowser title="Choose the game itself, not a launcher or anti-cheat helper" mode="file" extensions={["exe"]} start={g().trackingExe ?? g().installDir} onPick={(p) => void choose(p)} onClose={() => setBrowsing(false)} /></Show>
  </div>}</Show>;
}
