import { createResource, createSignal, For, onCleanup, Show } from "solid-js";
import { api, errorText, events, type LbHit } from "../api";
import { library } from "../state/library";
import { isLauncher } from "../state/launchers";
import { FileBrowser } from "./FileBrowser";
import { NavScope } from "./NavScope";
import { Picker } from "./Picker";

export function AddGame(props: { onClose: () => void; onAdded: (id: number) => Promise<void> }) {
  const [platform, setPlatform] = createSignal("");
  const [query, setQuery] = createSignal("");
  const [submitted, setSubmitted] = createSignal("");
  const [selected, setSelected] = createSignal<LbHit | null>(null);
  const [custom, setCustom] = createSignal(false);
  const [kind, setKind] = createSignal("emulator");
  const [file, setFile] = createSignal("");
  const [browsing, setBrowsing] = createSignal(false);
  const [busy, setBusy] = createSignal(false);
  const [updating, setUpdating] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [added, setAdded] = createSignal<number | null>(null);
  const [settings] = createResource(async () => {
    try {
      const s = await api.getSettings();
      const available = s.platforms.filter((p) => p.enabled && !isLauncher(p.id));
      setPlatform(available.find((p) => p.id === library.platform())?.id ?? available[0]?.id ?? "");
      return s;
    } catch (e) { setError(errorText(e)); return null; }
  });
  const [results, { refetch }] = createResource(
    () => platform() && submitted() ? { platform: platform(), query: submitted() } : false,
    async ({ platform, query }) => {
      try { return { hits: await api.searchGameCatalogue(platform, query), error: null }; }
      catch (e) { return { hits: [] as LbHit[], error: errorText(e) }; }
    },
  );
  const finished = events.onLbFinished(() => { setUpdating(false); if (submitted()) void refetch(); });
  const failed = events.onLbFailed((e) => { setUpdating(false); setError(e); });
  onCleanup(() => { void finished.then((stop) => stop()); void failed.then((stop) => stop()); });
  const close = () => { if (!busy()) props.onClose(); };
  const title = () => selected()?.name ?? (custom() ? submitted() : "");
  function search() {
    const repeated = query().trim() === submitted();
    setSelected(null); setCustom(false); setSubmitted(query().trim());
    if (repeated && submitted()) void refetch();
  }
  async function updateCatalogue() {
    setUpdating(true); setError(null);
    try { await api.fetchLaunchbox(true); }
    catch (e) { setUpdating(false); setError(errorText(e)); }
  }
  async function add() {
    if (busy()) return;
    setBusy(true); setError(null);
    try {
      const id = added() ?? await api.addManualGame(platform(), title(), selected()?.dbId ?? null, kind(), file() || null);
      setAdded(id);
      await props.onAdded(id);
    } catch (e) { setError(errorText(e)); }
    finally { setBusy(false); }
  }
  return <div class="overlay" onClick={(e) => e.target === e.currentTarget && close()}>
    <NavScope class="panel match-picker" modal onBack={close}>
      <h2>Add game</h2>
      <p class="dim">Choose a title from the LaunchBox Games Database, or enter your own. Game files are optional; you can attach them later in Game files.</p>
      <Show when={error()}><p class="warn">{error()}</p></Show>
      <fieldset disabled={busy() || added() !== null} style={{ border: "0", padding: "0", margin: "0", "min-width": "0" }}>
        <div class="row">
          <span>Platform</span>
          <Picker title="Game platform" value={platform()} options={(settings()?.platforms ?? []).filter((p) => p.enabled && !isLauncher(p.id)).map((p) => ({ value: p.id, label: p.name }))} onChange={(v) => { setPlatform(v); setSelected(null); setCustom(false); setFile(""); }} />
        </div>
        <div class="row">
          <input class="input grow" aria-label="Game title" placeholder="Search for a game…" value={query()} onInput={(e) => setQuery(e.currentTarget.value)} onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); search(); } }} />
          <button class="btn" disabled={!platform() || !query().trim() || results.loading} onClick={search}>Search LaunchBox</button>
        </div>
        <div class="row">
          <button class="btn small" disabled={updating() || settings()?.values.lbEnabled === false} onClick={() => void updateCatalogue()}>{updating() ? "Updating catalogue…" : "Update catalogue"}</button>
          <span class="dim small">Search uses KB's cached catalogue. {settings()?.values.lbEnabled === false ? "Enable LaunchBox in Settings → Game info to update it." : "Update loads missing catalogue data through the existing background download."}</span>
        </div>
        <Show when={results.loading}><p class="dim">Searching…</p></Show>
        <Show when={results()?.error}><p class="warn">{results()?.error}</p></Show>
        <div class="match-list" style={{ "max-height": "25vh", "overflow-y": "auto" }}>
          <For each={results.loading ? [] : results()?.hits ?? []}>{(hit) => <button class="match-option" aria-pressed={selected()?.dbId === hit.dbId} onClick={() => { setSelected(hit); setCustom(false); }}>
            <span class="match-text"><span class="match-name">{selected()?.dbId === hit.dbId ? "✓ " : ""}{hit.name}</span><Show when={hit.alsoKnownAs}><span class="dim small">Also known as {hit.alsoKnownAs}</span></Show></span>
          </button>}</For>
        </div>
        <Show when={submitted()}>
          <Show when={!results.loading && !results()?.hits.length}><p class="dim small">No catalogue results. Try another title, update the catalogue, or use your own title below.</p></Show>
          <button class="btn small" aria-pressed={custom()} onClick={() => { setCustom(true); setSelected(null); }}>{custom() ? "✓ " : ""}Use my title: {submitted()}</button>
        </Show>
        <h3>{title() ? `Selected: ${title()}` : "Select a result or use your own title"}</h3>
        <div class="row" style={{ "flex-wrap": "wrap" }}>
          <button class="btn" style={{ "white-space": "normal" }} role="switch" aria-checked={kind() !== "emulator"} onClick={() => { setKind(kind() === "emulator" ? "port" : "emulator"); setFile(""); }}>Launch an executable (port / recomp / decomp): {kind() === "emulator" ? "Off" : "On"}</button>
          <button class="btn" onClick={() => setBrowsing(true)}>{kind() === "emulator" ? "Choose game file…" : "Choose executable…"}</button>
          <Show when={file()}><button class="btn small" onClick={() => setFile("")}>Add without files</button></Show>
        </div>
        <p class="dim small">{kind() === "emulator" ? "Uses this platform's configured emulator automatically. Enable the option above for a .exe game." : "Starts your selected .exe directly, without an emulator."}</p>
        <p class="mono">{file() || "No file selected — add it later."}</p>
      </fieldset>
      <div class="panel-actions">
        <button class="btn primary" disabled={busy() || settings.loading || !platform() || !title()} onClick={() => void add()}>{busy() ? "Adding…" : added() ? "Open added game" : "Add game"}</button>
        <button class="btn" disabled={busy()} onClick={close}>Cancel</button>
      </div>
      <Show when={browsing()}><FileBrowser title={kind() === "emulator" ? "Choose the game's ROM or disc image" : "Choose the port, recomp or decomp executable"} mode="file" extensions={kind() === "emulator" ? (settings()?.platforms.find((p) => p.id === platform())?.extensionsOverride ?? settings()?.platforms.find((p) => p.id === platform())?.defaultExtensions ?? []) : ["exe"]} start={file()} onPick={(p) => { setFile(p); setBrowsing(false); }} onClose={() => setBrowsing(false)} /></Show>
    </NavScope>
  </div>;
}
