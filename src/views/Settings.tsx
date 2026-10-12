import { createResource, createSignal, For, onCleanup, Show } from "solid-js";
import { open } from "@tauri-apps/plugin-dialog";
import { api, errorText, type EmulatorRow, type ExitCombo, type PlatformRow, type Settings, type SourceRow } from "../api";
import { library } from "../state/library";
import { buttonLabel, COMBO_BUTTONS, session } from "../state/session";
import { pushHandler } from "../input/actions";

type Section = "library" | "emulators" | "platforms" | "general";

export function SettingsView(props: { onClose: () => void }) {
  const [settings, { refetch }] = createResource(api.getSettings);
  const [section, setSection] = createSignal<Section>("library");
  const [error, setError] = createSignal<string | null>(null);
  const [notice, setNotice] = createSignal<string | null>(null);

  onCleanup(
    pushHandler((a) => {
      if (a === "back" || a === "menu") {
        props.onClose();
        return true;
      }
      return false;
    }),
  );

  /** Runs a mutation, surfaces errors, refreshes settings (and the library if asked). */
  async function run(fn: () => Promise<unknown>, opts: { notice?: string; reloadLibrary?: boolean } = {}) {
    setError(null);
    try {
      await fn();
      if (opts.notice) setNotice(opts.notice);
      await refetch();
      if (opts.reloadLibrary) await library.reload();
    } catch (e) {
      setError(errorText(e));
      await refetch();
    }
  }

  const sections: [Section, string][] = [
    ["library", "Library"],
    ["emulators", "Emulators"],
    ["platforms", "Platforms"],
    ["general", "General"],
  ];

  return (
    <div class="settings">
      <header class="topbar">
        <h1>Settings</h1>
        <nav class="platform-tabs">
          <For each={sections}>
            {([id, label]) => (
              <button class="tab" classList={{ active: section() === id }} onClick={() => setSection(id)}>
                {label}
              </button>
            )}
          </For>
        </nav>
        <div class="topbar-right">
          <button class="btn ghost" onClick={props.onClose}>
            Done <kbd>Esc</kbd>
          </button>
        </div>
      </header>

      <div class="settings-body">
        <Show when={error()}>
          <div class="banner error" onClick={() => setError(null)}>
            {error()}
          </div>
        </Show>
        <Show when={notice()}>
          <div class="banner ok" onClick={() => setNotice(null)}>
            {notice()}
          </div>
        </Show>
        <Show when={settings()} fallback={<p>Loading…</p>}>
          {(s) => (
            <>
              <Show when={section() === "library"}>
                <LibrarySection s={s()} run={run} />
              </Show>
              <Show when={section() === "emulators"}>
                <EmulatorsSection s={s()} run={run} />
              </Show>
              <Show when={section() === "platforms"}>
                <PlatformsSection s={s()} run={run} />
              </Show>
              <Show when={section() === "general"}>
                <GeneralSection s={s()} run={run} />
              </Show>
            </>
          )}
        </Show>
      </div>
    </div>
  );
}

type Run = (fn: () => Promise<unknown>, opts?: { notice?: string; reloadLibrary?: boolean }) => Promise<void>;

async function pickFolder(): Promise<string | null> {
  const r = await open({ directory: true, multiple: false });
  return typeof r === "string" ? r : null;
}

// ------------------------------------------------------------------ library

function LibrarySection(props: { s: Settings; run: Run }) {
  const [newPath, setNewPath] = createSignal("");
  const [newPlatform, setNewPlatform] = createSignal<string>("");

  const add = () =>
    props.run(
      async () => {
        await api.addSource(newPath(), newPlatform() || null);
        setNewPath("");
      },
      { notice: "Folder added. Run a scan to pick up its games." },
    );

  return (
    <section class="card-section">
      <h2>Scan</h2>
      <div class="row">
        <button class="btn primary" disabled={!!library.scanProgress()} onClick={() => void library.scan()}>
          {library.scanProgress() ? "Scanning…" : "Scan library"}
        </button>
        <Show when={library.scanProgress()}>
          {(p) => (
            <span class="dim">
              Folder {p().done + 1} of {p().total}: <span class="mono">{p().source}</span>
            </span>
          )}
        </Show>
      </div>
      <Show when={library.scanError()}>
        <p class="warn">Scan failed: {library.scanError()}</p>
      </Show>
      <Show when={library.lastScan()}>
        {(s) => (
          <div class="scan-summary">
            <p>
              {s().gamesTotal} games · {s().added} added · {s().updated} updated · {s().missing} missing ·{" "}
              {(s().durationMs / 1000).toFixed(1)} s
            </p>
            <Show when={s().warnings.length > 0}>
              <details>
                <summary>{s().warnings.length} skipped / warnings</summary>
                <ul class="warnings">
                  <For each={s().warnings}>{(w) => <li class="mono">{w}</li>}</For>
                </ul>
              </details>
            </Show>
          </div>
        )}
      </Show>

      <h2>ROM folders</h2>
      <p class="dim">
        Assign a platform to each folder. Use <b>Auto</b> only for mixed folders: it decides by file
        extension and disc headers (GameCube vs Wii, Sega CD vs PS1) and skips files it can't place.
      </p>
      <table class="table">
        <thead>
          <tr>
            <th>Folder</th>
            <th>Platform</th>
            <th>Subfolders</th>
            <th />
          </tr>
        </thead>
        <tbody>
          <For each={props.s.sources}>{(src) => <SourceRowView src={src} s={props.s} run={props.run} />}</For>
        </tbody>
      </table>

      <div class="row add-row">
        <input
          class="input grow mono"
          placeholder="Folder path, e.g. E:\Emulator Games\Nintendo\GBA"
          value={newPath()}
          onInput={(e) => setNewPath(e.currentTarget.value)}
          onKeyDown={(e) => e.key === "Enter" && newPath().trim() && void add()}
        />
        <button class="btn" onClick={async () => { const p = await pickFolder(); if (p) setNewPath(p); }}>
          Browse…
        </button>
        <PlatformSelect platforms={props.s.platforms} value={newPlatform()} onChange={setNewPlatform} />
        <button class="btn primary" disabled={!newPath().trim()} onClick={() => void add()}>
          Add folder
        </button>
      </div>
      <div class="row">
        <button
          class="btn"
          onClick={() =>
            props.run(() => api.addRpcs3InstalledSource(), { notice: "Added RPCS3's installed games folder." })
          }
        >
          Add RPCS3 installed games (dev_hdd0)
        </button>
      </div>
    </section>
  );
}

function PlatformSelect(props: { platforms: PlatformRow[]; value: string; onChange: (v: string) => void }) {
  return (
    <select class="input" value={props.value} onChange={(e) => props.onChange(e.currentTarget.value)}>
      <option value="">Auto</option>
      <For each={props.platforms.filter((p) => p.enabled)}>{(p) => <option value={p.id}>{p.name}</option>}</For>
    </select>
  );
}

function SourceRowView(props: { src: SourceRow; s: Settings; run: Run }) {
  const hdd0 = () => props.src.kind === "ps3_hdd0";
  return (
    <tr classList={{ missing: !props.src.exists }}>
      <td class="mono">
        {props.src.path}
        <Show when={!props.src.exists}>
          <span class="badge warn-badge">not found</span>
        </Show>
      </td>
      <td>
        <Show when={!hdd0()} fallback={<span class="dim">PS3 (installed)</span>}>
          <PlatformSelect
            platforms={props.s.platforms}
            value={props.src.platformId ?? ""}
            onChange={(v) => props.run(() => api.updateSource(props.src.id, v || null, props.src.recursive))}
          />
        </Show>
      </td>
      <td>
        <Show when={!hdd0()}>
          <input
            type="checkbox"
            checked={props.src.recursive}
            onChange={(e) =>
              props.run(() => api.updateSource(props.src.id, props.src.platformId, e.currentTarget.checked))
            }
          />
        </Show>
      </td>
      <td>
        <button
          class="btn small danger"
          onClick={() => props.run(() => api.removeSource(props.src.id), { reloadLibrary: true })}
        >
          Remove
        </button>
      </td>
    </tr>
  );
}

// ------------------------------------------------------------------ emulators

function EmulatorsSection(props: { s: Settings; run: Run }) {
  const coresDir = () => (props.s.values.retroarchCoresDir as string | undefined) ?? "";

  const detect = async () => {
    const folder = await pickFolder();
    if (!folder) return;
    await props.run(async () => {
      const found = await api.detectEmulators(folder);
      if (found.length === 0) throw "No known emulator executables found in that folder.";
    }, { notice: "Emulator paths filled in from the folder." });
  };

  return (
    <section class="card-section">
      <div class="row">
        <button class="btn primary" onClick={() => void detect()}>
          Find emulators in a folder…
        </button>
        <span class="dim">Looks for the six supported emulators up to two folders deep.</span>
      </div>
      <For each={props.s.emulators}>
        {(emu) => <EmulatorEditor emu={emu} run={props.run} />}
      </For>
      <h2>RetroArch cores folder</h2>
      <PathField
        value={coresDir()}
        placeholder="e.g. E:\Emulators\RetroArch\cores"
        directory
        onCommit={(v) => props.run(() => api.setSetting("retroarchCoresDir", v))}
      />
    </section>
  );
}

function EmulatorEditor(props: { emu: EmulatorRow; run: Run }) {
  const save = (exe: string | null, tpl: string | null) =>
    props.run(() => api.updateEmulator(props.emu.id, exe, tpl));
  return (
    <div class="emu">
      <h2>
        {props.emu.name}
        <span class="status" classList={{ ok: props.emu.exeExists }}>
          {props.emu.exeExists ? "found" : props.emu.exePath ? "file missing" : "not set"}
        </span>
      </h2>
      <label class="field-label">Executable</label>
      <PathField
        value={props.emu.exePath ?? ""}
        placeholder="Path to the emulator .exe"
        onCommit={(v) => save(v || null, props.emu.templateOverride)}
      />
      <label class="field-label">Launch template</label>
      <input
        class="input mono"
        value={props.emu.templateOverride ?? ""}
        placeholder={props.emu.defaultTemplate}
        onChange={(e) => save(props.emu.exePath, e.currentTarget.value || null)}
      />
      <p class="dim small">
        Leave empty for the default. Placeholders: <code>{"{emulator}"}</code> <code>{"{core}"}</code>{" "}
        <code>{"{rom}"}</code> <code>{"{rom_dir}"}</code> <code>{"{rom_name}"}</code>
      </p>
    </div>
  );
}

function PathField(props: { value: string; placeholder: string; directory?: boolean; onCommit: (v: string) => void }) {
  let input!: HTMLInputElement;
  const browse = async () => {
    const r = props.directory
      ? await open({ directory: true })
      : await open({ filters: [{ name: "Executable", extensions: ["exe"] }] });
    if (typeof r === "string") {
      input.value = r;
      props.onCommit(r);
    }
  };
  return (
    <div class="row">
      <input
        ref={input}
        class="input grow mono"
        value={props.value}
        placeholder={props.placeholder}
        onChange={(e) => props.onCommit(e.currentTarget.value.trim())}
      />
      <button class="btn" onClick={() => void browse()}>
        Browse…
      </button>
    </div>
  );
}

// ------------------------------------------------------------------ platforms

function PlatformsSection(props: { s: Settings; run: Run }) {
  const emuName = (id: string) => props.s.emulators.find((e) => e.id === id)?.name ?? id;
  const emuTemplate = (id: string) => {
    const e = props.s.emulators.find((x) => x.id === id);
    return e?.templateOverride ?? e?.defaultTemplate ?? "";
  };

  const save = (p: PlatformRow, patch: Partial<Omit<PlatformRow, "id">>) => {
    const next = { ...p, ...patch };
    return props.run(
      () =>
        api.updatePlatform({
          id: p.id,
          coreOverride: next.coreOverride,
          templateOverride: next.templateOverride,
          extensionsOverride: next.extensionsOverride,
          enabled: next.enabled,
        }),
      { reloadLibrary: patch.enabled !== undefined },
    );
  };

  const parseExts = (s: string) => {
    const list = s.split(/[\s,;]+/).map((x) => x.replace(/^\./, "").toLowerCase()).filter(Boolean);
    return list.length ? list : null;
  };

  return (
    <section class="card-section">
      <p class="dim">
        Overrides are optional; empty fields use the defaults shown in grey. Extension changes apply on the next
        scan.
      </p>
      <table class="table platforms-table">
        <thead>
          <tr>
            <th>On</th>
            <th>Platform</th>
            <th>Emulator</th>
            <th>Core</th>
            <th>Launch template</th>
            <th>Extensions</th>
            <th>Games</th>
          </tr>
        </thead>
        <tbody>
          <For each={props.s.platforms}>
            {(p) => (
              <tr classList={{ disabled: !p.enabled }}>
                <td>
                  <input type="checkbox" checked={p.enabled} onChange={(e) => save(p, { enabled: e.currentTarget.checked })} />
                </td>
                <td>{p.name}</td>
                <td>{emuName(p.emulatorId)}</td>
                <td>
                  <Show when={p.emulatorId === "retroarch"} fallback={<span class="dim">—</span>}>
                    <input
                      class="input mono narrow"
                      value={p.coreOverride ?? ""}
                      placeholder={p.defaultCore ?? ""}
                      onChange={(e) => save(p, { coreOverride: e.currentTarget.value.trim() || null })}
                    />
                  </Show>
                </td>
                <td>
                  <input
                    class="input mono"
                    value={p.templateOverride ?? ""}
                    placeholder={emuTemplate(p.emulatorId)}
                    onChange={(e) => save(p, { templateOverride: e.currentTarget.value.trim() || null })}
                  />
                </td>
                <td>
                  <Show when={p.id !== "ps3"} fallback={<span class="dim">folders (PS3_GAME)</span>}>
                    <input
                      class="input mono"
                      value={p.extensionsOverride?.join(", ") ?? ""}
                      placeholder={p.defaultExtensions.join(", ")}
                      onChange={(e) => save(p, { extensionsOverride: parseExts(e.currentTarget.value) })}
                    />
                  </Show>
                </td>
                <td class="num">{p.gameCount}</td>
              </tr>
            )}
          </For>
        </tbody>
      </table>
    </section>
  );
}

// ------------------------------------------------------------------ general

function GeneralSection(props: { s: Settings; run: Run }) {
  const combo = (): ExitCombo =>
    (props.s.values.exitCombo as ExitCombo | undefined) ?? { buttons: ["back", "start"], holdMs: 1000 };
  const family = () => session.lastPad()?.family;
  const toggle = (b: string, on: boolean) => {
    const buttons = on ? [...combo().buttons, b] : combo().buttons.filter((x) => x !== b);
    return props.run(() => api.setSetting("exitCombo", { ...combo(), buttons }));
  };

  return (
    <section class="card-section">
      <h2>Exit-emulator combo</h2>
      <p class="dim">
        Hold these buttons together on one controller to close the running game. It works while the emulator has
        focus. Pick at least two, and avoid combos your games use.
        {session.lastPad() ? ` Labels shown for: ${session.lastPad()!.name}.` : ""}
      </p>
      <div class="combo-grid">
        <For each={COMBO_BUTTONS}>
          {(b) => (
            <label class="check">
              <input
                type="checkbox"
                checked={combo().buttons.includes(b)}
                onChange={(e) => void toggle(b, e.currentTarget.checked)}
              />
              {buttonLabel(b, family())}
            </label>
          )}
        </For>
      </div>
      <div class="row">
        <span>Hold for</span>
        <select
          class="input"
          value={String(combo().holdMs)}
          onChange={(e) => props.run(() => api.setSetting("exitCombo", { ...combo(), holdMs: Number(e.currentTarget.value) }))}
        >
          <For each={[500, 1000, 1500, 2000, 3000]}>{(ms) => <option value={ms}>{ms / 1000} s</option>}</For>
        </select>
      </div>

      <h2>Display</h2>
      <label class="check">
        <input
          type="checkbox"
          checked={props.s.values.tvMode === true}
          onChange={(e) => props.run(() => api.setSetting("tvMode", e.currentTarget.checked))}
        />
        Start in fullscreen TV mode
      </label>
      <p class="dim small">
        <kbd>F11</kbd> toggles fullscreen at any time.
      </p>
    </section>
  );
}
