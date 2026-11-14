import { createResource, createSignal, Show } from "solid-js";
import { api, errorText, type DirectLaunch } from "../api";
import { NavScope } from "./NavScope";
import { library } from "../state/library";
import { FileBrowser } from "./FileBrowser";

/** Per-game override: switching back to Emulator keeps the original ROM and emulator choice. */
export function DirectLaunchSetting(props: { id: number; launchPath: string; manual?: boolean; onRemoved: () => void; onChange: () => void }) {
  const [kind, setKind] = createSignal<DirectLaunch["kind"] | "emulator">("emulator");
  const [executable, setExecutable] = createSignal("");
  const [error, setError] = createSignal<string | null>(null);
  const [browsing, setBrowsing] = createSignal(false);
  const [saving, setSaving] = createSignal(false);
  const [choosingFile, setChoosingFile] = createSignal(false);
  const [removing, setRemoving] = createSignal(false);
  const [loadError, setLoadError] = createSignal<string | null>(null);
  const [config, { refetch }] = createResource(() => props.id, async (id) => {
    setLoadError(null);
    const value = await api.getDirectLaunch(id).catch((e) => { setLoadError(errorText(e)); return null; });
    setKind(value?.kind ?? "emulator");
    setExecutable(value?.executable ?? (/\.exe$/i.test(props.launchPath) ? props.launchPath : ""));
    setError(null);
    return value;
  });
  const dirty = () => kind() !== (config()?.kind ?? "emulator") || (kind() !== "emulator" && executable() !== config()?.executable);
  async function save() {
    if (saving()) return;
    setSaving(true); setError(null);
    try {
      const type = kind();
      await api.setDirectLaunch(props.id, type === "emulator" ? null : { kind: type, executable: executable() });
      await refetch();
      props.onChange();
    } catch (e) { setError(errorText(e)); }
    finally { setSaving(false); }
  }
  return <div>
    <Show when={props.manual}>
      <div class="row tight">
        <button class="btn small" disabled={saving()} onClick={() => setChoosingFile(true)}>Choose game file for emulator…</button>
        <button class="btn small" disabled={saving()} onClick={() => setRemoving(true)}>Remove library entry…</button>
      </div>
      <Show when={props.launchPath.startsWith("kb-manual:") && !config()?.executable}><p class="dim small">Added without game files. Choose a ROM for emulation, or enable executable launching below.</p></Show>
    </Show>
    <h3>Launch method</h3>
    <Show when={loadError()}><p class="warn">{loadError()}</p></Show>
    <div class="row tight" style={{ "flex-wrap": "wrap" }}>
      <button class="btn" style={{ "white-space": "normal" }} role="switch" aria-checked={kind() !== "emulator"} disabled={config.loading || !!loadError() || saving()} onClick={() => setKind(kind() === "emulator" ? config()?.kind ?? "port" : "emulator")}>
        Launch from an executable: {kind() === "emulator" ? "Off" : "On"}
      </button>
      <Show when={kind() !== "emulator"}>
        <button class="btn small" disabled={saving()} onClick={() => setBrowsing(true)}>Choose game executable…</button>
      </Show>
      <button class="btn small" disabled={config.loading || !!loadError() || saving() || !dirty() || (kind() !== "emulator" && !executable() && !props.manual)} onClick={() => void save()}>{saving() ? "Saving…" : "Apply"}</button>
    </div>
    <Show when={kind() !== "emulator"}>
      <p class="dim small">Launches the selected .exe directly, without an emulator. Choose the game itself, not an installer or launcher. Its own folder is used as the working folder.</p>
      <p class="mono">{executable() || "Choose an executable, then Apply."}</p>
    </Show>
    <Show when={kind() === "emulator"}><p class="dim small">Uses this platform's configured emulator automatically.</p></Show>
    <Show when={dirty()}><p class="dim small">Changes take effect after Apply.</p></Show>
    <Show when={error()}><p class="warn">{error()}</p></Show>
    <Show when={choosingFile()}><FileBrowser title="Choose the game file for the platform's emulator" mode="file" start={props.launchPath.startsWith("kb-manual:") ? null : props.launchPath} onPick={(path) => {
      setChoosingFile(false); setSaving(true); setError(null);
      void api.setManualGameFile(props.id, path).then(() => { props.onChange(); }).catch((e) => setError(errorText(e))).finally(() => setSaving(false));
    }} onClose={() => setChoosingFile(false)} /></Show>
    <Show when={removing()}><div class="overlay"><NavScope class="panel" modal onBack={() => { if (!saving()) setRemoving(false); }}>
      <h3>Remove this library entry?</h3><p>This removes its KB play history, favorites and artwork choices. Your game files stay on disk.</p>
      <button class="btn" disabled={saving()} onClick={() => setRemoving(false)}>Cancel</button>
      <button class="btn danger" disabled={saving()} onClick={() => {
        setSaving(true); setError(null);
        void api.removeManualGame(props.id).then(async () => { await library.reload(); props.onRemoved(); }).catch((e) => { setRemoving(false); setError(errorText(e)); }).finally(() => setSaving(false));
      }}>Remove entry</button>
    </NavScope></div></Show>
    <Show when={browsing()}><FileBrowser title="Choose the port, recomp or decomp game executable" mode="file" extensions={["exe"]} start={executable() || (props.launchPath.startsWith("kb-manual:") ? null : props.launchPath)} onPick={(path) => { setExecutable(path); setBrowsing(false); }} onClose={() => setBrowsing(false)} /></Show>
  </div>;
}
