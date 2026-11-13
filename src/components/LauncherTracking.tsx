import { createResource, createSignal, Show } from "solid-js";
import { api, errorText, type SteamGame } from "../api";
import { launcherName } from "../state/launchers";
import { FileBrowser } from "./FileBrowser";

/** The picker changes process tracking only; the client still launches the game. */
export function LauncherTracking(props: { id: number; provider: string }) {
  const [game, { refetch }] = createResource(() => ({ id: props.id, provider: props.provider }), async ({ id, provider }): Promise<Pick<SteamGame, "installDir" | "executables" | "trackingExe"> | null> => provider === "steam" ? api.steamGame(id) : api.launcherGame(id));
  const [browsing, setBrowsing] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const choose = async (path: string | null) => {
    setBrowsing(false); setError(null);
    try { await (props.provider === "steam" ? api.setSteamTracking(props.id, path) : api.setLauncherTracking(props.id, path)); await refetch(); }
    catch (e) { setError(errorText(e)); }
  };
  return <Show when={game()}>{(g) => <div class="steam-tracking">
    <p class="dim small">Launches through {launcherName(props.provider)} · {g().trackingExe ? "Tracking your chosen executable" : g().executables.length ? "Using automatic game tracking" : "Choose the game executable to enable safe tracking"}</p>
    <button class="btn small" onClick={() => setBrowsing(true)}>Choose game executable…</button>
    <Show when={g().trackingExe}><button class="btn small" onClick={() => void choose(null)}>Use automatic tracking</button></Show>
    <Show when={error()}><p class="warn">{error()}</p></Show>
    <Show when={browsing()}><FileBrowser title="Choose the game itself, not a launcher or anti-cheat helper" mode="file" extensions={["exe"]} start={g().trackingExe ?? g().installDir} onPick={(p) => void choose(p)} onClose={() => setBrowsing(false)} /></Show>
  </div>}</Show>;
}
