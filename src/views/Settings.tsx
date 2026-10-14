import { createResource, createSignal, For, onMount, Show } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import { api, errorText, type EmulatorRow, type ExitCombo, type PlatformRow, type Settings, type SourceRow } from "../api";
import { library } from "../state/library";
import { updates } from "../state/updates";
import { buttonLabel, COMBO_BUTTONS, session } from "../state/session";
import { NavScope } from "../components/NavScope";
import { Picker } from "../components/Picker";
import { FileBrowser } from "../components/FileBrowser";
import { Hint } from "../components/Hints";
import { ScanReport, summarizeChanges } from "../components/ScanReport";

type Section = "library" | "emulators" | "platforms" | "services" | "general";
const SECTIONS: [Section, string][] = [
  ["library", "Library"],
  ["emulators", "Emulators"],
  ["platforms", "Platforms"],
  ["services", "Game info"],
  ["general", "General"],
];

type Run = (fn: () => Promise<unknown>, opts?: { notice?: string; reloadLibrary?: boolean }) => Promise<void>;

export function SettingsView(props: { onClose: () => void }) {
  // A store reconciled by id: after each change only the edited row updates,
  // so the focused control (and controller navigation) stays put.
  const [store, setStore] = createStore<{ s: Settings | null }>({ s: null });
  const [section, setSection] = createSignal<Section>("library");
  const [error, setError] = createSignal<string | null>(null);
  const [notice, setNotice] = createSignal<string | null>(null);

  const refresh = async () => setStore("s", reconcile(await api.getSettings(), { key: "id" }));
  onMount(() => void refresh().catch((e) => setError(errorText(e))));

  const run: Run = async (fn, opts = {}) => {
    setError(null);
    setNotice(null);
    try {
      await fn();
      if (opts.notice) setNotice(opts.notice);
      if (opts.reloadLibrary) await library.reload();
    } catch (e) {
      setError(errorText(e));
    }
    await refresh();
  };

  const cycle = (d: 1 | -1) => {
    const i = SECTIONS.findIndex(([id]) => id === section());
    setSection(SECTIONS[(i + d + SECTIONS.length) % SECTIONS.length][0]);
  };

  return (
    <NavScope class="settings" onBack={props.onClose} onMenu={props.onClose} onTab={cycle}>
      <header class="topbar">
        <h1>Settings</h1>
        <nav class="platform-tabs">
          <For each={SECTIONS}>
            {([id, label]) => (
              <button class="tab" classList={{ active: section() === id }} onClick={() => setSection(id)}>
                {label}
              </button>
            )}
          </For>
        </nav>
        <div class="topbar-right">
          <button class="btn ghost" onClick={props.onClose}>
            Done
          </button>
        </div>
      </header>

      <div class="settings-body">
        <Show when={error()}>
          <div class="banner error" role="alert">
            {error()}
          </div>
        </Show>
        <Show when={notice()}>
          <div class="banner ok" role="status">
            {notice()}
          </div>
        </Show>
        <Show when={store.s} fallback={<p>Loading…</p>}>
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
              <Show when={section() === "services"}>
                <ServicesSection s={s()} run={run} />
                <SteamGridDbSection run={run} />
                <RetroAchievementsSection run={run} />
              </Show>
              <Show when={section() === "general"}>
                <GeneralSection s={s()} run={run} />
              </Show>
            </>
          )}
        </Show>
      </div>
      <footer class="hints">
        <Hint actions={["prevPlatform", "nextPlatform"]} label="Section" />
        <Hint action="select" label="Select / edit" />
        <Hint action="back" label="Back" />
      </footer>
    </NavScope>
  );
}

function platformOptions(platforms: PlatformRow[]) {
  return [
    { value: "", label: "Auto (mixed folder)" },
    ...platforms.filter((p) => p.enabled).map((p) => ({ value: p.id, label: p.name })),
  ];
}

// ------------------------------------------------------------------ library

function LibrarySection(props: { s: Settings; run: Run }) {
  const platformName = (id: string) => props.s.platforms.find((p) => p.id === id)?.name ?? id.toUpperCase();
  const [newPath, setNewPath] = createSignal("");
  const [newPlatform, setNewPlatform] = createSignal("");
  // Once the user picks a platform themselves, stop auto-suggesting.
  const [platformChosen, setPlatformChosen] = createSignal(false);
  const [browsing, setBrowsing] = createSignal(false);

  /** Sets the path and preselects the platform its folder name suggests ("PS2" -> PlayStation 2). */
  const changePath = async (p: string) => {
    setNewPath(p);
    if (platformChosen()) return;
    const suggested = p.trim() ? await api.suggestPlatform(p).catch(() => null) : null;
    if (newPath() === p) setNewPlatform(suggested ?? "");
  };

  const add = () =>
    props.run(
      async () => {
        await api.addSource(newPath(), newPlatform() || null);
        setNewPath("");
        setNewPlatform("");
        setPlatformChosen(false);
        await library.scan(); // pick up the new folder's games right away
      },
      { notice: "Folder added. Scanning it now…" },
    );

  return (
    <section class="card-section">
      <h2>Scan</h2>
      <div class="row">
        <Show
          when={library.scanProgress()}
          fallback={
            <button class="btn primary" onClick={() => void library.scan()}>
              Scan library
            </button>
          }
        >
          {(p) => (
            <>
              <button class="btn" onClick={() => void api.cancelScan()}>
                Cancel scan
              </button>
              <span class="dim">
                Folder {Math.min(p().done + 1, Math.max(p().total, 1))} of {Math.max(p().total, 1)}
                {p().filesTotal > 0 ? ` · ${p().filesDone.toLocaleString()} / ${p().filesTotal.toLocaleString()} files` : ""}
                : <span class="mono">{p().source}</span>
              </span>
            </>
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
              {s().cancelled ? "Scan cancelled · " : ""}
              {s().gamesTotal} games · {s().added} added · {s().updated} updated
              {s().removed > 0 ? ` · ${s().removed} removed` : ""}
              {s().missing > 0 ? ` · ${s().missing} missing (kept: played or favorite)` : ""} ·{" "}
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
      <Show when={library.report()} keyed>
        {(r) => (
          <div class="scan-summary">
            <p>
              <strong>{r.title}:</strong>{" "}
              {r.changes.length > 0 ? summarizeChanges(r.changes, platformName) : "no games added or removed."}
            </p>
            <Show when={r.changes.length > 0}>
              <ScanReport changes={r.changes} name={platformName} />
            </Show>
          </div>
        )}
      </Show>

      <h2>Covers</h2>
      <p class="dim">
        Box art comes from libretro-thumbnails (matched by game name) and GameTDB (looked up by the game's ID, which
        covers PS3 and fills gaps for GameCube, Wii, DS and 3DS). No account needed. Everything is cached, so covers
        stay available offline. Change any cover from the game's details.
      </p>
      <div class="row">
        <Show
          when={library.artProgress()}
          fallback={
            <button class="btn" disabled={!!library.scanProgress()} onClick={() => void library.fetchArt()}>
              Fetch missing covers
            </button>
          }
        >
          {(p) => (
            <>
              <button class="btn" onClick={() => void api.cancelScan()}>
                Stop
              </button>
              <span class="dim">
                Fetching covers: {p().done} / {p().total} · {p().found} found
              </span>
            </>
          )}
        </Show>
        <Show when={!library.artProgress() && library.lastArt()}>
          {(s) => (
            <span class="dim">
              Last run: {s().found} found · {s().notFound} not found
              {s().failed > 0 ? ` · ${s().failed} failed (retried next time)` : ""}
            </span>
          )}
        </Show>
      </div>
      <label class="check">
        <input
          type="checkbox"
          checked={props.s.values.autoArt !== false}
          onChange={(e) => props.run(() => api.setSetting("autoArt", e.currentTarget.checked))}
        />
        Fetch covers automatically after each scan
      </label>

      <h2>ROM folders</h2>
      <p class="dim">
        <b>Auto</b> works out each game's platform from its file type and disc contents (GameCube/Wii, PS1/PS2/PSP,
        Sega CD), falling back to folder names like "PS2" or "PSX". Files it still can't place are listed under the
        scan's warnings; assigning a platform to the folder always works.
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
          aria-label="Folder path"
          placeholder="Folder path, e.g. E:\Emulator Games\Nintendo\GBA"
          value={newPath()}
          onInput={(e) => void changePath(e.currentTarget.value)}
          onKeyDown={(e) => e.key === "Enter" && newPath().trim() && void add()}
        />
        <button class="btn" onClick={() => setBrowsing(true)}>
          Browse…
        </button>
        <Picker
          title="Platform for this folder"
          value={newPlatform()}
          options={platformOptions(props.s.platforms)}
          onChange={(v) => {
            setNewPlatform(v);
            setPlatformChosen(true);
          }}
        />
        <button class="btn primary" disabled={!newPath().trim()} onClick={() => void add()}>
          Add folder
        </button>
      </div>
      <div class="row">
        <button
          class="btn"
          onClick={() =>
            props.run(
              async () => {
                await api.addRpcs3InstalledSource();
                await library.scan();
              },
              { notice: "Added RPCS3's installed games. Scanning them now…" },
            )
          }
        >
          Add RPCS3 installed games (dev_hdd0)
        </button>
      </div>
      <Show when={browsing()}>
        <FileBrowser
          title="Choose a ROM folder"
          mode="folder"
          start={newPath() || props.s.sources.at(-1)?.path}
          onPick={(p) => {
            void changePath(p);
            setBrowsing(false);
          }}
          onClose={() => setBrowsing(false)}
        />
      </Show>
    </section>
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
          <Picker
            title="Platform for this folder"
            value={props.src.platformId ?? ""}
            options={platformOptions(props.s.platforms)}
            onChange={(v) =>
              props.run(async () => {
                await api.updateSource(props.src.id, v || null, props.src.recursive);
                await library.scan(); // re-place this folder's games
              })
            }
          />
        </Show>
      </td>
      <td>
        <Show when={!hdd0()}>
          <input
            type="checkbox"
            aria-label="Include subfolders"
            checked={props.src.recursive}
            onChange={(e) => {
              const recursive = e.currentTarget.checked;
              return props.run(async () => {
                await api.updateSource(props.src.id, props.src.platformId, recursive);
                await library.scan();
              });
            }}
          />
        </Show>
      </td>
      <td>
        <button
          class="btn small danger"
          onClick={() => props.run(() => library.removeSource(props.src.id))}
        >
          Remove
        </button>
      </td>
    </tr>
  );
}

// ------------------------------------------------------------------ emulators

function EmulatorsSection(props: { s: Settings; run: Run }) {
  const [browsing, setBrowsing] = createSignal(false);
  const coresDir = () => (props.s.values.retroarchCoresDir as string | undefined) ?? "";

  const detect = (folder: string) =>
    props.run(
      async () => {
        const found = await api.detectEmulators(folder);
        if (found.length === 0) throw "No known emulator executables found in that folder.";
      },
      { notice: "Emulator paths filled in from the folder." },
    );

  return (
    <section class="card-section">
      <div class="row">
        <button class="btn primary" onClick={() => setBrowsing(true)}>
          Find emulators in a folder…
        </button>
        <span class="dim">Looks for the six supported emulators up to two folders deep.</span>
      </div>
      <For each={props.s.emulators}>{(emu) => <EmulatorEditor emu={emu} run={props.run} />}</For>
      <h2>RetroArch cores folder</h2>
      <PathField
        label="RetroArch cores folder"
        value={coresDir()}
        placeholder="e.g. E:\Emulators\RetroArch\cores"
        mode="folder"
        onCommit={(v) => props.run(() => api.setSetting("retroarchCoresDir", v))}
      />
      <Show when={browsing()}>
        <FileBrowser
          title="Folder containing your emulators"
          mode="folder"
          start={props.s.emulators.find((e) => e.exePath)?.exePath}
          onPick={(p) => {
            setBrowsing(false);
            void detect(p);
          }}
          onClose={() => setBrowsing(false)}
        />
      </Show>
    </section>
  );
}

function EmulatorEditor(props: { emu: EmulatorRow; run: Run }) {
  const save = (exe: string | null, tpl: string | null) => props.run(() => api.updateEmulator(props.emu.id, exe, tpl));
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
        label={`${props.emu.name} executable`}
        value={props.emu.exePath ?? ""}
        placeholder="Path to the emulator .exe"
        mode="file"
        onCommit={(v) => save(v || null, props.emu.templateOverride)}
      />
      <label class="field-label">Launch template</label>
      <input
        class="input mono"
        aria-label={`${props.emu.name} launch template`}
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

function PathField(props: {
  label: string;
  value: string;
  placeholder: string;
  mode: "folder" | "file";
  onCommit: (v: string) => void;
}) {
  const [browsing, setBrowsing] = createSignal(false);
  return (
    <div class="row">
      <input
        class="input grow mono"
        aria-label={props.label}
        value={props.value}
        placeholder={props.placeholder}
        onChange={(e) => props.onCommit(e.currentTarget.value.trim())}
      />
      <button class="btn" onClick={() => setBrowsing(true)}>
        Browse…
      </button>
      <Show when={browsing()}>
        <FileBrowser
          title={props.label}
          mode={props.mode}
          extensions={["exe"]}
          start={props.value}
          onPick={(p) => {
            setBrowsing(false);
            props.onCommit(p);
          }}
          onClose={() => setBrowsing(false)}
        />
      </Show>
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
        Overrides are optional; empty fields use the defaults shown in grey. Extension changes apply on the next scan.
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
                  <input
                    type="checkbox"
                    aria-label={`Enable ${p.name}`}
                    checked={p.enabled}
                    onChange={(e) => save(p, { enabled: e.currentTarget.checked })}
                  />
                </td>
                <td>{p.name}</td>
                <td>{emuName(p.emulatorId)}</td>
                <td>
                  <Show when={p.emulatorId === "retroarch"} fallback={<span class="dim">—</span>}>
                    <input
                      class="input mono narrow"
                      aria-label={`${p.name} core`}
                      value={p.coreOverride ?? ""}
                      placeholder={p.defaultCore ?? ""}
                      onChange={(e) => save(p, { coreOverride: e.currentTarget.value.trim() || null })}
                    />
                  </Show>
                </td>
                <td>
                  <input
                    class="input mono"
                    aria-label={`${p.name} launch template`}
                    value={p.templateOverride ?? ""}
                    placeholder={emuTemplate(p.emulatorId)}
                    onChange={(e) => save(p, { templateOverride: e.currentTarget.value.trim() || null })}
                  />
                </td>
                <td>
                  <Show when={p.id !== "ps3"} fallback={<span class="dim">folders (PS3_GAME)</span>}>
                    <input
                      class="input mono"
                      aria-label={`${p.name} extensions`}
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

// ------------------------------------------------------------------ services

function ServicesSection(props: { s: Settings; run: Run }) {
  const [services, { refetch }] = createResource(() => api.getServices());
  const [clientId, setClientId] = createSignal("");
  const [secret, setSecret] = createSignal("");

  const save = () =>
    props.run(
      async () => {
        await api.setIgdbKeys(clientId(), secret());
        setClientId("");
        setSecret("");
        await refetch();
      },
      { notice: "Connected to IGDB. Looking up your games now…" },
    );
  const remove = () =>
    props.run(async () => {
      await api.removeIgdbKeys();
      await refetch();
    });

  return (
    <section class="card-section">
      <h2>IGDB (optional)</h2>
      <p class="dim">
        Adds descriptions, release dates, developers, publishers and genres (with a genre filter in the library).
        Covers work without it. IGDB is free for personal use, but each person needs their own key from Twitch.
        Your keys stay in KB's folder, encrypted with your Windows login.
      </p>
      <Show when={services()}>
        {(sv) => (
          <Show
            when={sv().igdb}
            fallback={
              <>
                <ol class="steps">
                  <li>
                    Sign in at <span class="mono">dev.twitch.tv/console</span> with a free Twitch account (Twitch asks
                    you to turn on two-factor authentication first).
                  </li>
                  <li>
                    Choose <b>Register Your Application</b>: any name, OAuth Redirect URL{" "}
                    <span class="mono">http://localhost</span>, category <b>Application Integration</b>, client type{" "}
                    <b>Confidential</b>.
                  </li>
                  <li>
                    Open the app you created: copy the <b>Client ID</b>, then press <b>New Secret</b> and copy the{" "}
                    <b>Client Secret</b>.
                  </li>
                  <li>Paste both here and press Save.</li>
                </ol>
                <div class="form-grid">
                  <label for="igdb-id">Client ID</label>
                  <input
                    id="igdb-id"
                    class="input mono"
                    autocomplete="off"
                    spellcheck={false}
                    value={clientId()}
                    onInput={(e) => setClientId(e.currentTarget.value)}
                  />
                  <label for="igdb-secret">Client Secret</label>
                  <input
                    id="igdb-secret"
                    class="input mono"
                    type="password"
                    autocomplete="off"
                    value={secret()}
                    onInput={(e) => setSecret(e.currentTarget.value)}
                  />
                </div>
                <div class="row">
                  <button class="btn primary" disabled={!clientId().trim() || !secret().trim()} onClick={() => void save()}>
                    Save and connect
                  </button>
                </div>
              </>
            }
          >
            <p>
              <span class="ok-text">Connected</span>{" "}
              <span class="dim">(Client ID {sv().igdbClientId})</span>
            </p>
            <div class="row">
              <Show
                when={library.metaProgress()}
                fallback={
                  <button class="btn" disabled={!!library.scanProgress()} onClick={() => void api.fetchMetadata()}>
                    Look up missing info
                  </button>
                }
              >
                {(p) => (
                  <>
                    <button class="btn" onClick={() => void api.cancelScan()}>
                      Stop
                    </button>
                    <span class="dim">
                      Looking up games: {p().done} / {p().total} · {p().matched} found
                    </span>
                  </>
                )}
              </Show>
              <Show when={!library.metaProgress() && library.lastMeta()}>
                {(m) => (
                  <span class="dim">
                    Last run: {m().matched} found · {m().notFound} not found
                    {m().failed > 0 ? ` · ${m().failed} failed (retried next time)` : ""}
                  </span>
                )}
              </Show>
              <button class="btn danger" onClick={() => void remove()}>
                Remove keys
              </button>
            </div>
            <Show when={library.metaError()}>
              <p class="warn">{library.metaError()}</p>
            </Show>
            <label class="check">
              <input
                type="checkbox"
                checked={props.s.values.autoMeta !== false}
                onChange={(e) => props.run(() => api.setSetting("autoMeta", e.currentTarget.checked))}
              />
              Look up new games automatically after each scan
            </label>
            <p class="dim small">
              Wrong game? Open it and use <b>Fix match…</b> to pick the right one. Games IGDB doesn't know are tried
              again weekly. Info from IGDB.com, used under the Twitch Developer Services Agreement.
            </p>
          </Show>
        )}
      </Show>
    </section>
  );
}

function SteamGridDbSection(props: { run: Run }) {
  const [services, { refetch }] = createResource(() => api.getServices());
  const [key, setKey] = createSignal("");

  const save = () =>
    props.run(
      async () => {
        await api.setSgdbKey(key());
        setKey("");
        await refetch();
      },
      { notice: "Connected to SteamGridDB. Fetching backgrounds and logos now…" },
    );
  const remove = () =>
    props.run(async () => {
      await api.removeSgdbKey();
      await refetch();
    });

  return (
    <section class="card-section">
      <h2>SteamGridDB (optional)</h2>
      <p class="dim">
        Community artwork: a wide background and a logo for each game (shown in TV Mode), more covers to choose from,
        and covers for games the other sources don't have. Free with an account. Your key stays in KB's folder,
        encrypted with your Windows login.
      </p>
      <Show when={services()}>
        {(sv) => (
          <Show
            when={sv().sgdb}
            fallback={
              <>
                <ol class="steps">
                  <li>
                    Sign in at <span class="mono">steamgriddb.com</span> (with Steam).
                  </li>
                  <li>
                    Open your preferences, <span class="mono">steamgriddb.com/profile/preferences</span>, and generate
                    an API key in its API section.
                  </li>
                  <li>Paste it here and press Save.</li>
                </ol>
                <div class="form-grid">
                  <label for="sgdb-key">API key</label>
                  <input
                    id="sgdb-key"
                    class="input mono"
                    type="password"
                    autocomplete="off"
                    value={key()}
                    onInput={(e) => setKey(e.currentTarget.value)}
                  />
                </div>
                <div class="row">
                  <button class="btn primary" disabled={!key().trim()} onClick={() => void save()}>
                    Save and connect
                  </button>
                </div>
              </>
            }
          >
            <p>
              <span class="ok-text">Connected</span>
            </p>
            <div class="row">
              <Show
                when={library.sgdbProgress()}
                fallback={
                  <button class="btn" disabled={!!library.scanProgress()} onClick={() => void api.fetchExtras()}>
                    Look up missing artwork
                  </button>
                }
              >
                {(p) => (
                  <>
                    <button class="btn" onClick={() => void api.cancelScan()}>
                      Stop
                    </button>
                    <span class="dim">
                      Looking up games: {p().done} / {p().total} · {p().matched} found
                    </span>
                  </>
                )}
              </Show>
              <Show when={!library.sgdbProgress() && library.lastSgdb()}>
                {(s) => (
                  <span class="dim">
                    Last run: {s().matched} found ({s().heroes} backgrounds, {s().logos} logos, {s().covers} covers)
                    {s().notFound > 0 ? ` · ${s().notFound} not found` : ""}
                  </span>
                )}
              </Show>
              <button class="btn danger" onClick={() => void remove()}>
                Remove key
              </button>
            </div>
            <Show when={library.sgdbError()}>
              <p class="warn">{library.sgdbError()}</p>
            </Show>
            <p class="dim small">
              Pick other pictures from a game's details (Change cover / background / logo). Games SteamGridDB doesn't
              know are tried again weekly. Artwork from SteamGridDB.com, made by its community.
            </p>
          </Show>
        )}
      </Show>
    </section>
  );
}

function RetroAchievementsSection(props: { run: Run }) {
  const [services, { refetch }] = createResource(() => api.getServices());
  const [user, setUser] = createSignal("");
  const [key, setKey] = createSignal("");

  const save = () =>
    props.run(
      async () => {
        await api.setRaKeys(user(), key());
        setUser("");
        setKey("");
        await refetch();
      },
      { notice: "Connected to RetroAchievements. Identifying your games now…" },
    );
  const remove = () =>
    props.run(async () => {
      await api.removeRaKeys();
      await refetch();
    });

  return (
    <section class="card-section">
      <h2>RetroAchievements (optional)</h2>
      <p class="dim">
        Shows each game's achievements and which ones you've unlocked, on its details page. Games are recognized by
        RetroAchievements' own fingerprint of the game file, so it works whatever the files are called. Consoles
        RetroAchievements doesn't cover (like 3DS) simply show nothing. Your key stays in KB's folder, encrypted
        with your Windows login.
      </p>
      <Show when={services()}>
        {(sv) => (
          <Show
            when={sv().raUser}
            fallback={
              <>
                <ol class="steps">
                  <li>
                    Sign in at <span class="mono">retroachievements.org</span> (a free account; the one you use in
                    your emulators).
                  </li>
                  <li>
                    Open your settings page, <span class="mono">retroachievements.org/settings</span>, and copy your{" "}
                    <b>Web API Key</b>. Treat it like a password.
                  </li>
                  <li>Enter your username and the key here and press Save.</li>
                </ol>
                <div class="form-grid">
                  <label for="ra-user">Username</label>
                  <input
                    id="ra-user"
                    class="input"
                    autocomplete="off"
                    spellcheck={false}
                    value={user()}
                    onInput={(e) => setUser(e.currentTarget.value)}
                  />
                  <label for="ra-key">Web API Key</label>
                  <input
                    id="ra-key"
                    class="input mono"
                    type="password"
                    autocomplete="off"
                    value={key()}
                    onInput={(e) => setKey(e.currentTarget.value)}
                  />
                </div>
                <div class="row">
                  <button class="btn primary" disabled={!user().trim() || !key().trim()} onClick={() => void save()}>
                    Save and connect
                  </button>
                </div>
              </>
            }
          >
            <p>
              <span class="ok-text">Connected</span> <span class="dim">as {sv().raUser}</span>
            </p>
            <div class="row">
              <Show when={library.raProgress()}>
                {(p) => (
                  <>
                    <button class="btn" onClick={() => void api.cancelScan()}>
                      Stop
                    </button>
                    <span class="dim">
                      Identifying games: {p().done} / {p().total} · {p().matched} recognized
                    </span>
                  </>
                )}
              </Show>
              <Show when={!library.raProgress() && library.lastRa()}>
                {(s) => (
                  <span class="dim">
                    Last run: {s().matched} recognized
                    {s().unknown > 0 ? ` · ${s().unknown} not recognized` : ""}
                  </span>
                )}
              </Show>
              <button class="btn danger" onClick={() => void remove()}>
                Remove key
              </button>
            </div>
            <Show when={library.raError()}>
              <p class="warn">{library.raError()}</p>
            </Show>
            <p class="dim small">
              A game "not recognized" means RetroAchievements doesn't list that exact dump (a hack, a translation or
              a different revision). Achievement data from RetroAchievements.org.
            </p>
          </Show>
        )}
      </Show>
    </section>
  );
}

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
        <Picker
          title="Hold the combo for"
          value={String(combo().holdMs)}
          options={[500, 1000, 1500, 2000, 3000].map((ms) => ({ value: String(ms), label: `${ms / 1000} s` }))}
          onChange={(v) => props.run(() => api.setSetting("exitCombo", { ...combo(), holdMs: Number(v) }))}
        />
      </div>

      <h2>Display</h2>
      <label class="check">
        <input
          type="checkbox"
          checked={library.grouped()}
          onChange={(e) => library.setGrouped(e.currentTarget.checked)}
        />
        Show All, Favorites and search results in sections per platform
      </label>
      <p class="dim small">
        Each platform starts its own rows, so covers in a row share a shape. Switch from the library with the right
        stick button (R3) or V. Zoom with the right stick, + / −, or Ctrl + mouse wheel.
      </p>
      <label class="check">
        <input
          type="checkbox"
          checked={props.s.values.tvMode === true}
          onChange={(e) => props.run(() => api.setSetting("tvMode", e.currentTarget.checked))}
        />
        Start fullscreen
      </label>
      <label class="check">
        <input
          type="checkbox"
          checked={props.s.values.startInTv === true}
          onChange={(e) => props.run(() => api.setSetting("startInTv", e.currentTarget.checked))}
        />
        Start in TV Mode
      </label>
      <div class="row">
        <span>Interface size</span>
        <Picker
          title="Interface size"
          value={props.s.values.interfaceSize === "fixed" ? "fixed" : "auto"}
          options={[
            { value: "auto", label: "Fit the screen" },
            { value: "fixed", label: "Follow Windows scaling only" },
          ]}
          onChange={(v) =>
            props.run(async () => {
              await api.setSetting("interfaceSize", v);
              await api.refreshZoom();
            })
          }
        />
      </div>
      <p class="dim small">
        "Fit the screen" makes big screens at low Windows scaling (like a 4K TV at 100%) look like 1080p instead of
        tiny. Screens up to 1080p, or with Windows scaling turned up, look the same either way.
      </p>
      <p class="dim small">
        <kbd>F11</kbd> toggles fullscreen at any time. TV Mode is the big-screen view: switch with the left stick button
        (L3), <kbd>F10</kbd> or the TV Mode button. It shows each game's background and logo when SteamGridDB is set up
        (above), and a blurred cover otherwise.
      </p>

      <TvModeBlock s={props.s} run={props.run} />

      <UpdatesBlock s={props.s} run={props.run} />
      <StorageBlock run={props.run} />
    </section>
  );
}

const size = (n: number) =>
  n >= 1073741824 ? `${(n / 1073741824).toFixed(1)} GB` : n >= 1048576 ? `${(n / 1048576).toFixed(0)} MB` : `${Math.round(n / 1024)} KB`;

function StorageBlock(props: { run: Run }) {
  const [storage, { refetch }] = createResource(() => api.getStorage().catch(() => null));
  const [asking, setAsking] = createSignal<"clear" | "reset" | null>(null);
  const [forgetKeys, setForgetKeys] = createSignal(false);
  const [working, setWorking] = createSignal(false);
  const [failed, setFailed] = createSignal<string | null>(null);

  const ask = (what: "clear" | "reset") => {
    setForgetKeys(false);
    setFailed(null);
    setAsking(what);
  };
  const close = () => !working() && setAsking(null);

  async function confirm() {
    setWorking(true);
    setFailed(null);
    try {
      if (asking() === "reset") {
        await api.resetApp(forgetKeys());
        location.reload();
        return;
      }
      const freed = await api.clearDownloaded();
      setAsking(null);
      await props.run(() => library.reload(), {
        notice: `Downloaded data cleared (${size(freed)} freed). KB is fetching it again in the background.`,
      });
      await refetch();
    } catch (e) {
      setFailed(errorText(e));
    } finally {
      setWorking(false);
    }
  }

  return (
    <>
      <h2>Storage and reset</h2>
      <p>
        Downloaded data: <span class="mono">{storage() ? size(storage()!.downloadedBytes) : "…"}</span>{" "}
        <span class="dim small">(covers, backgrounds, logos, game info, achievement data)</span>
      </p>
      <div class="row">
        <button class="btn" onClick={() => ask("clear")}>
          Clear downloaded data…
        </button>
        <button class="btn danger" onClick={() => ask("reset")}>
          Reset KaneegaBell…
        </button>
      </div>
      <p class="dim small">
        Your games, emulators and saves are never touched: both only delete inside KB's own folder
        {storage() ? <> (<span class="mono">{storage()!.dataDir}</span>)</> : null}.
      </p>
      <Show when={asking()}>
        {(what) => (
          <div class="overlay" onClick={(e) => e.target === e.currentTarget && close()}>
            <NavScope class="panel confirm-panel" modal onBack={close}>
              <Show
                when={what() === "reset"}
                fallback={
                  <>
                    <h2>Clear downloaded data?</h2>
                    <p>
                      KB deletes the covers, backgrounds, logos, game info and achievement data it downloaded, then
                      downloads them again in the background. Use this if artwork or info looks wrong or out of date.
                    </p>
                    <p class="dim">
                      Kept: your games, play time, favorites, folders, settings and keys, plus the covers, backgrounds,
                      logos and matches you picked yourself.
                    </p>
                  </>
                }
              >
                <h2>Reset KaneegaBell?</h2>
                <p>
                  KB goes back to how it was on the first start. It forgets your library, play time, favorites, ROM
                  folders, emulator paths and every setting, and deletes the downloaded data.
                </p>
                <p class="dim">
                  Your game files, emulators and saves are not touched. A copy of the old library is kept in KB's
                  folder as <span class="mono">library.before-reset.db</span>, in case you change your mind.
                </p>
                <label class="check">
                  <input type="checkbox" checked={forgetKeys()} onChange={(e) => setForgetKeys(e.currentTarget.checked)} />
                  Also remove my saved keys (IGDB, SteamGridDB, RetroAchievements)
                </label>
              </Show>
              <Show when={failed()}>
                <p class="warn">{failed()}</p>
              </Show>
              <div class="row">
                <button class="btn" data-autofocus disabled={working()} onClick={close}>
                  Cancel
                </button>
                <button class="btn danger" disabled={working()} onClick={() => void confirm()}>
                  {working() ? "Working…" : what() === "reset" ? "Reset KaneegaBell" : "Clear downloaded data"}
                </button>
              </div>
              <div class="panel-actions">
                <div class="hints-row">
                  <Hint action="select" label="Choose" />
                  <Hint action="back" label="Cancel" />
                </div>
              </div>
            </NavScope>
          </div>
        )}
      </Show>
    </>
  );
}

function TvModeBlock(props: { s: Settings; run: Run }) {
  // Asked fresh each time Settings opens, so a TV that was just turned on shows up.
  const [displays] = createResource(() => api.listDisplays().catch(() => []));
  const chosen = () => (props.s.values.tvDisplay as string | undefined) ?? "";
  const displayOptions = () => {
    const list = displays() ?? [];
    const opts = [{ value: "", label: "The display KB is on" }];
    for (const d of list) {
      const notes = [d.primary ? "main" : "", d.current ? "KB is here" : ""].filter(Boolean).join(", ");
      opts.push({ value: d.id, label: `${d.name} (${d.width}×${d.height}${notes ? `, ${notes}` : ""})` });
    }
    // The chosen TV is off or unplugged right now: keep it selectable.
    if (chosen() && !list.some((d) => d.id === chosen())) opts.push({ value: chosen(), label: "The TV chosen before (not connected now)" });
    return opts;
  };
  const set = (key: string, v: unknown) => props.run(() => api.setSetting(key, v));

  return (
    <>
      <h2>TV Mode</h2>
      <div class="form-grid">
        <label>Display</label>
        <Picker title="Open TV Mode on" value={chosen()} options={displayOptions()} onChange={(v) => set("tvDisplay", v || null)} />
        <label>Text size</label>
        <Picker
          title="TV Mode text size"
          value={(props.s.values.tvTextSize as string | undefined) ?? "m"}
          options={[
            { value: "s", label: "Small" },
            { value: "m", label: "Medium" },
            { value: "l", label: "Large" },
            { value: "xl", label: "Extra large" },
          ]}
          onChange={(v) => set("tvTextSize", v)}
        />
        <label>Overscan margin</label>
        <Picker
          title="Overscan margin"
          value={String((props.s.values.tvOverscan as number | undefined) ?? 0)}
          options={[
            { value: "0", label: "Off" },
            { value: "2.5", label: "2.5%" },
            { value: "5", label: "5%" },
            { value: "7.5", label: "7.5%" },
          ]}
          onChange={(v) => set("tvOverscan", Number(v))}
        />
      </div>
      <p class="dim small">
        With a display chosen, TV Mode moves KB to it and goes fullscreen, and leaving TV Mode brings the window back. If
        that display is off or unplugged, TV Mode opens where KB is. Raise the overscan margin if your TV cuts off the
        edges of the picture (the clock or the hints along the bottom are missing).
      </p>
    </>
  );
}

function UpdatesBlock(props: { s: Settings; run: Run }) {
  const [info] = createResource(() => api.appInfo().catch(() => null));
  const [checking, setChecking] = createSignal(false);
  const [result, setResult] = createSignal<string | null>(null);

  async function checkNow() {
    setChecking(true);
    setResult(null);
    try {
      // When there is one, the update dialog opens by itself.
      if (!(await updates.checkNow())) setResult("You have the latest version.");
    } catch (e) {
      setResult(errorText(e));
    } finally {
      setChecking(false);
    }
  }

  return (
    <>
      <h2>Updates</h2>
      <p>
        KaneegaBell <span class="mono">{info()?.version ?? ""}</span>
      </p>
      <label class="check">
        <input
          type="checkbox"
          checked={props.s.values.autoUpdateCheck !== false}
          onChange={(e) => props.run(() => api.setSetting("autoUpdateCheck", e.currentTarget.checked))}
        />
        Check for updates when KaneegaBell starts
      </label>
      <div class="row">
        <button class="btn" disabled={checking()} onClick={() => void checkNow()}>
          {checking() ? "Checking…" : "Check now"}
        </button>
        <Show when={result()}>
          <span class="dim">{result()}</span>
        </Show>
      </div>
      <p class="dim small">
        Updates come from KaneegaBell's GitHub releases and are only installed when you choose to, never while a game
        is running. Each one is signed, and KB refuses any download that isn't.
        {info()?.devBuild ? " This copy runs from a build folder, so it doesn't check by itself; use Check now." : ""}
      </p>
    </>
  );
}
