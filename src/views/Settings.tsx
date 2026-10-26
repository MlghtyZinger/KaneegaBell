import { createEffect, createResource, createSignal, For, on, onCleanup, onMount, Show, type JSX } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import { api, errorText, type EmulatorRow, type ExitCombo, type PlatformRow, type Settings, type SourceRow } from "../api";
import { library } from "../state/library";
import { updates } from "../state/updates";
import { whatsNew } from "../state/whatsNew";
import { buttonLabel, COMBO_BUTTONS, session } from "../state/session";
import { NavScope } from "../components/NavScope";
import { Picker } from "../components/Picker";
import { FileBrowser } from "../components/FileBrowser";
import { Hint } from "../components/Hints";
import { ScanReport, summarizeChanges } from "../components/ScanReport";
import { device } from "../input/actions";
import { editWithOsk } from "../state/osk";
import { EmulatorBadge } from "../components/Icons";
import { applyTheme, FOCUS_LOOKS } from "../state/theme";
import { fx, INTROS, type AnimLevel, type IntroKey, type IntroPick, type SoundProfile } from "../state/fx";
import { playIntro } from "../fx/Intro";
import { sfx } from "../fx/audio";

type Run = (fn: () => Promise<unknown>, opts?: { notice?: string; reloadLibrary?: boolean }) => Promise<void>;

/**
 * Every setting's name and explanation. The explanation pops up when the setting
 * has focus, and search highlights a setting whose name, `words` or explanation
 * match.
 */
type Info = { title: string; detail: string; words?: string };
const INFO: Record<string, Info> = {
  scan: { title: "Scan", words: "rescan games warnings skipped", detail: "Looks through your ROM folders for new, changed and removed games. The report lists what changed and anything it skipped." },
  covers: { title: "Covers", words: "box art artwork thumbnails libretro gametdb automatic", detail: "Box art comes first from the LaunchBox Games Database, then libretro-thumbnails and GameTDB. No account needed, and it's cached for offline use. Change any cover from the game's details. The switch fetches covers after each scan." },
  folders: { title: "ROM folders", words: "auto folder source path platform subfolders add browse rpcs3 ps3 dev_hdd0 installed psn", detail: "Auto works out each game's platform from its files and disc contents, falling back to folder names like \"PS2\" or \"PSX\". Files it can't place are listed under the scan's warnings; assigning a platform to the folder always works. Add RPCS3 installed games adds the PS3 games in RPCS3's dev_hdd0 folder as a source of their own." },
  findEmus: { title: "Find emulators", words: "detect scan folder exe paths", detail: "Looks in a folder (up to two levels deep) for every emulator KB knows, like RetroArch, Dolphin, PCSX2, Eden, Ryubing or xemu, and fills in their paths." },
  platforms: { title: "Platforms", words: "core template extensions file types override enable disable", detail: "Overrides are optional; empty fields use the defaults shown in grey. Core and launch template are for each platform's built-in emulator; emulators you add have their own. Extension changes apply on the next scan." },
  launchbox: { title: "LaunchBox Games Database", words: "covers logos backgrounds info description genres free no account", detail: "Covers, logos, backgrounds, back covers, discs and spines, descriptions, release dates, developers and genres, without a key. It comes first; anything you pick yourself always stays. KB downloads its database (about 100 MB) once and keeps it in its folder, so adding games never downloads it again; it keeps only your games' details and fetches pictures just for your games. Data from the LaunchBox Games Database community." },
  wikidata: { title: "Wikidata", words: "series franchise collections free no account", detail: "Which series and franchise each game belongs to, shown in the game details and Collections. Asked once per platform with games, then only for new games or, monthly, for games it couldn't find. Free data (CC0)." },
  igdb: { title: "IGDB", words: "twitch key client id secret metadata genres fix match", detail: "Optional. Descriptions, release dates, developers, publishers and genres from IGDB, plus Fix match. Free for personal use with your own key from Twitch (dev.twitch.tv/console). Keys stay in KB's folder, encrypted with your Windows login." },
  sgdb: { title: "SteamGridDB", words: "key backgrounds heroes logos covers artwork", detail: "Optional. Community artwork: wide backgrounds and logos (TV Mode, sidebar), more covers to choose from, and covers the other sources lack. Free key from steamgriddb.com/profile/preferences." },
  ra: { title: "RetroAchievements", words: "achievements key username badges hardcore", detail: "Optional. Each game's achievements and your unlocks. Games are recognized by RetroAchievements' own fingerprint of the file, whatever it's called. Needs your username and Web API key from retroachievements.org/settings." },
  style: { title: "Style", words: "theme colors colours look appearance dark light contrast focus icon ember midnight daylight moss storm", detail: "KaneegaBell's colors, corner shape and how the focused game is marked. The window and taskbar icon change with it. Built-in styles: Ember (the default), Midnight, Daylight, High contrast, Moss and Storm." },
  exitCombo: { title: "Exit combo", words: "close game buttons hold controller quit", detail: "Hold these buttons together on one controller to close the running game, even while the emulator has focus. Pick at least two, and avoid combos your games use. KB asks the emulator to close, then ends it after 4 s." },
  opensIn: { title: "KB opens in", words: "start fullscreen window tv mode big picture f11 l3 f10", detail: "How KB looks when it starts: in a window, fullscreen, or in TV Mode, the big-screen view with backgrounds and logos. F11 toggles fullscreen and L3 or F10 switches TV Mode at any time." },
  intro: { title: "Start-up intro", words: "boot animation splash konami rockstar red dead redemption rdr1 playstation gamecube switch random sound file intros folder", detail: "An intro plays when KB starts, on the desktop and in TV Mode. Random never repeats the last one; Off skips it. Your own sound files go in the data folder's intros folder." },
  introWatch: { title: "Watch an intro", words: "preview play intro", detail: "Pick an intro and play it to see it." },
  animDesktop: { title: "Animations, desktop", words: "motion reduced off", detail: "Full moves the most, Reduced only fades, Off keeps everything still." },
  animTv: { title: "Animations, TV Mode", words: "motion reduced off", detail: "Full moves the most, Reduced only fades, Off keeps everything still." },
  launchFx: { title: "Launch animation", words: "case cartridge 3d disc game start", detail: "Opens the game's case (or puts the cartridge in its console) before the emulator starts, on the desktop and in TV Mode. Back cancels it." },
  soundProfile: { title: "Sound profile", words: "sound effects audio tactile console retro menu clicks", detail: "The short sounds for moving, choosing, going back and starting a game. Silent while a game runs." },
  soundDesktop: { title: "Sounds, desktop", words: "volume mute audio", detail: "Menu sounds on the desktop: off, or how loud." },
  soundTv: { title: "Sounds, TV Mode", words: "volume mute audio", detail: "Menu sounds in TV Mode: off, or how loud." },
  tvText: { title: "TV Mode text size", words: "small large couch", detail: "Text size in TV Mode, for reading from the couch." },
  tvShelf: { title: "TV Mode selected game", words: "3d case flat cover shelf", detail: "TV Mode shows the selected game as a large case you can turn and flip, or a flat cover." },
  size: { title: "Interface size", words: "zoom scaling dpi 4k display", detail: "\"Fit the screen\" makes big screens at low Windows scaling (like a 4K TV at 100%) look like 1080p instead of tiny. Screens up to 1080p, or with scaling turned up, look the same either way." },
  updates: { title: "Updates", words: "version check what's new access code", detail: "Updates install only when you choose to, never while a game runs, and KB refuses any download that isn't signed. The access code lets this copy read KaneegaBell's private releases." },
  storage: { title: "Storage and reset", words: "clear downloaded data reset backup cache space", detail: "Clear downloaded data deletes the covers, info and achievement data KB downloaded, then fetches them again. Reset goes back to the first start (a copy of the old library is kept). Neither touches your games, emulators or saves." },
  removed: { title: "Removed games", words: "forget missing hidden deleted play time", detail: "When a game's file disappears, KB keeps it hidden if you played or favorited it, so it comes back with the file (e.g. an unplugged drive). Forgetting deletes them for good. With the switch on, scans do that right away." },
};

/**
 * One setting as a tile: name and status, then its main button(s) and its
 * on/off switch, each in a fixed slot so they line up down the column. Lists
 * and forms (`sub`) open in a panel from the `open` button. Its explanation
 * pops up when it has focus or the mouse rests on it. With `tile` it is a
 * compact name-and-dot button instead (emulators, platforms), and everything
 * it has opens in its panel, the switch included.
 */
function Row(props: {
  k: string;
  title?: string;
  detail?: string;
  words?: string;
  /** What a compact tile says, when shorter than its title (the title still names its panel). */
  label?: string;
  status?: JSX.Element;
  children?: JSX.Element;
  toggle?: JSX.Element;
  toggleLabel?: string;
  open?: string | false;
  sub?: JSX.Element;
  tile?: boolean;
  dot?: "ok" | "off" | "warn";
}) {
  const title = () => props.title ?? INFO[props.k]?.title ?? "";
  const detail = () => props.detail ?? INFO[props.k]?.detail ?? "";
  const words = () => props.words ?? INFO[props.k]?.words ?? "";
  const [editing, setEditing] = createSignal(false);
  // Checked by key: reading a JSX prop would build it a second time.
  const hasStatus = "status" in props;
  const hasToggle = "toggle" in props;
  return (
    <Show
      when={props.tile}
      fallback={
        <div class="set-item" id={`set-${props.k}`} data-title={title()} data-detail={detail()} data-words={words()}>
          <div class="set-text">
            <div class="set-name">{title()}</div>
            {hasStatus && <div class="set-status">{props.status}</div>}
          </div>
          <div class="set-actions">
            {props.children}
            <Show when={props.open}>
              <button class="btn" onClick={() => setEditing(true)}>
                {props.open}
              </button>
            </Show>
          </div>
          <div class="set-toggle">{props.toggle}</div>
          <Show when={editing()}>
            <SetPanel title={title()} onClose={() => setEditing(false)}>
              {props.sub}
            </SetPanel>
          </Show>
        </div>
      }
    >
      <div class="set-tile" id={`set-${props.k}`} data-title={title()} data-detail={detail()} data-words={words()}>
        <button class="set-tile-btn" title={title()} onClick={() => setEditing(true)}>
          <span class="set-dot" classList={{ ok: props.dot === "ok", warn: props.dot === "warn" }} aria-hidden="true" />
          <span class="set-tile-name">{props.label ?? title()}</span>
        </button>
        <Show when={editing()}>
          <SetPanel title={title()} onClose={() => setEditing(false)}>
            {hasStatus && <div class="set-status">{props.status}</div>}
            {props.sub}
            {hasToggle && (
              <div class="form-grid">
                <label>{props.toggleLabel ?? "On"}</label>
                {props.toggle}
              </div>
            )}
          </SetPanel>
        </Show>
      </div>
    </Show>
  );
}

/** A setting's list or form, over the section. */
function SetPanel(props: { title: string; onClose: () => void; children?: JSX.Element }) {
  return (
    <div class="overlay" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <NavScope class="panel set-panel" modal onBack={props.onClose}>
        <h2>{props.title}</h2>
        {props.children}
        <div class="panel-actions">
          <div class="hints-row">
            <Hint action="select" label="Choose" />
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

/** An on/off switch (a styled checkbox, so A / Enter / a click toggles it). */
function OnOff(props: { label: string; checked: boolean; onChange: (on: boolean) => void }) {
  return (
    <input
      type="checkbox"
      class="switch"
      role="switch"
      aria-label={props.label}
      checked={props.checked}
      onChange={(e) => props.onChange(e.currentTarget.checked)}
    />
  );
}

/** A missed game, as Settings lists it. */
type Missed = { title: string; platform: string; ambiguous?: boolean };
const SHOWN = 8;

/**
 * Names the games a source couldn't find (and, separately, those with several
 * possible matches), so "2 not found" says which two. Long lists show the
 * first few with a button for the rest.
 */
function MissedGames(props: { games: Missed[]; label?: string }) {
  const [all, setAll] = createSignal(false);
  const groups = () =>
    [
      [props.label ?? "Not found", props.games.filter((g) => !g.ambiguous)],
      ["Several possible matches (use Fix match)", props.games.filter((g) => g.ambiguous)],
    ] as const;
  return (
    <For each={groups().filter(([, list]) => list.length > 0)}>
      {([label, list]) => (
        <p class="missed small">
          <span class="dim">{label}: </span>
          {(all() ? list : list.slice(0, SHOWN)).map((g) => `${g.title} (${g.platform})`).join(" · ")}
          <Show when={!all() && list.length > SHOWN}>
            {" "}
            <button class="btn ghost small" onClick={() => setAll(true)}>
              and {list.length - SHOWN} more
            </button>
          </Show>
        </p>
      )}
    </For>
  );
}

/** A titled card of settings on the one-screen Settings view. */
function Group(props: { title: string; children: JSX.Element }) {
  return (
    <section class="set-group">
      <h2 class="set-group-title">{props.title}</h2>
      <div class="set-list">{props.children}</div>
    </section>
  );
}

export function SettingsView(props: { onClose: () => void }) {
  // A store reconciled by id: after each change only the edited row updates,
  // so the focused control (and controller navigation) stays put.
  const [store, setStore] = createStore<{ s: Settings | null }>({ s: null });
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

  // Search: a setting whose name, keywords, explanation or status match lights up
  // where it is; every other setting dims. Nothing moves or hides.
  const [query, setQuery] = createSignal("");
  let searchEl!: HTMLInputElement;
  let bodyEl!: HTMLDivElement;
  const items = () => [...bodyEl.querySelectorAll<HTMLElement>(".set-item, .set-tile")];
  const highlight = () => {
    const words = query().toLowerCase().split(/\s+/).filter(Boolean);
    for (const el of items()) {
      const hay = `${el.dataset.title ?? ""} ${el.dataset.words ?? ""} ${el.dataset.detail ?? ""} ${el.textContent ?? ""}`.toLowerCase();
      const hit = words.length > 0 && words.every((w) => hay.includes(w));
      el.classList.toggle("hit", hit);
      el.classList.toggle("dim", words.length > 0 && !hit);
    }
  };
  // After the rows have rendered (and again whenever a setting changes).
  createEffect(on([query, () => store.s], () => queueMicrotask(highlight)));
  const focusIn = (el: HTMLElement | undefined) => {
    el?.scrollIntoView({ block: "nearest" });
    el?.querySelector<HTMLElement>("button, input, [tabindex]")?.focus({ preventScroll: true });
  };
  /** Moves focus into the first matching setting. */
  const goToHit = () => focusIn(items().find((el) => el.classList.contains("hit")));
  const openSearch = () => {
    searchEl.focus();
    if (device() === "gamepad") editWithOsk(searchEl);
  };
  /** L1 / R1: the next or previous group. */
  const step = (d: 1 | -1) => {
    const groups = [...bodyEl.querySelectorAll<HTMLElement>(".set-group")];
    const at = groups.findIndex((g) => g.contains(document.activeElement));
    focusIn(groups[(at + d + groups.length) % groups.length]);
  };

  // A setting's explanation pops up beside it once focus or the mouse rests on it.
  const [shown, setShown] = createSignal<{ title: string; detail: string; x: number; y: number; above: boolean } | null>(null);
  let popEl: HTMLElement | null = null;
  let popTimer: number | undefined;
  const showFor = (el: HTMLElement | null, delay = 350) => {
    if (el === popEl) return;
    clearTimeout(popTimer);
    popEl = el;
    setShown(null);
    if (!el?.dataset.detail) return;
    popTimer = window.setTimeout(() => {
      const r = el.getBoundingClientRect();
      const above = r.bottom + 170 > window.innerHeight;
      setShown({
        title: el.dataset.title ?? "",
        detail: el.dataset.detail!,
        x: Math.max(16, Math.min(r.left + 12, window.innerWidth - 456)),
        y: above ? r.top - 6 : r.bottom + 6,
        above,
      });
    }, delay);
  };
  onCleanup(() => clearTimeout(popTimer));
  const follow = (e: Event) => {
    const t = e.target as HTMLElement;
    // Panels over the screen explain themselves.
    showFor(t.closest(".overlay") ? null : t.closest<HTMLElement>("[data-detail]"));
  };

  return (
    <NavScope
      class="settings"
      onBack={() => (query() ? setQuery("") : props.onClose())}
      onMenu={props.onClose}
      onTab={step}
      onSecondary={openSearch}
    >
      <header class="topbar">
        <h1>Settings</h1>
        <label class="settings-search">
          <span aria-hidden="true">⌕</span>
          <input
            ref={searchEl}
            type="search"
            aria-label="Search settings"
            placeholder="Search settings (e.g. bios, xbox, key)"
            value={query()}
            onInput={(e) => setQuery(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape" && query()) {
                setQuery("");
                e.stopPropagation();
              } else if (e.key === "Enter") {
                goToHit();
              }
            }}
          />
        </label>
        <div class="topbar-right">
          <button class="btn ghost" onClick={props.onClose}>
            Done
          </button>
        </div>
      </header>

      <div class="settings-body" ref={bodyEl} onFocusIn={follow} onMouseOver={follow} onMouseLeave={() => showFor(null)} onScroll={() => showFor(null)}>
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
            <div class="set-one">
              <div class="set-col">
                <Group title="Library">
                  <LibrarySection s={s()} run={run} />
                </Group>
                <Group title="Game info">
                  <LaunchBoxSection run={run} />
                  <WikidataSection run={run} />
                  <ServicesSection s={s()} run={run} />
                  <SteamGridDbSection run={run} />
                  <RetroAchievementsSection run={run} />
                </Group>
                <Group title="Controller">
                  <ControllerRows s={s()} run={run} />
                </Group>
              </div>
              <div class="set-col">
                <Group title="Emulators">
                  <EmulatorsSection s={s()} run={run} />
                </Group>
                <Group title="Platforms">
                  <PlatformsSection s={s()} run={run} />
                </Group>
                <Group title="TV Mode">
                  <TvRows s={s()} run={run} />
                </Group>
                <Group title="App">
                  <UpdatesBlock s={s()} run={run} />
                  <StorageBlock s={s()} run={run} />
                </Group>
              </div>
              <div class="set-col">
                <GeneralSection s={s()} run={run} />
              </div>
            </div>
          )}
        </Show>
      </div>
      <Show when={shown()}>
        {(d) => (
          <div
            class="set-pop"
            role="tooltip"
            style={{ left: `${d().x}px`, top: `${d().y}px`, transform: d().above ? "translateY(-100%)" : undefined }}
          >
            <b>{d().title}</b>
            <span>{d().detail}</span>
          </div>
        )}
      </Show>
      <footer class="hints">
        <Hint action="secondary" label="Search" />
        <Hint actions={["prevPlatform", "nextPlatform"]} label="Group" />
        <Hint action="select" label="Select / edit" />
        <Hint action="back" label={query() ? "Clear search" : "Back"} />
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
  // Games still without a cover, from the library already loaded (no query).
  const coverless = () => {
    const short = new Map(library.platforms().map((p) => [p.id, p.shortName]));
    return library
      .games()
      .filter((g) => !g.art)
      .map((g) => ({ title: g.title, platform: short.get(g.platformId) ?? g.platformId.toUpperCase() }));
  };
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
    <section class="card-section set-list">
      <Row
        k="scan"
        open={(!!library.scanError() || !!library.lastScan()?.warnings.length || !!library.report()) && "Report…"}
        status={
          <Show
            when={!library.scanError()}
            fallback={<span class="warn">Scan failed: {library.scanError()}</span>}
          >
          <Show
            when={library.scanProgress()}
            fallback={
              <Show when={library.lastScan()} fallback={`${library.games().length} games`}>
                {(s) => (
                  <>
                    {s().cancelled ? "Scan cancelled · " : ""}
                    {s().gamesTotal} games · {s().added} added · {s().updated} updated
                    {s().removed > 0 ? ` · ${s().removed} removed` : ""}
                    {s().missing > 0 ? ` · ${s().missing} missing (kept)` : ""} · {(s().durationMs / 1000).toFixed(1)} s
                  </>
                )}
              </Show>
            }
          >
            {(p) => (
              <>
                Folder {Math.min(p().done + 1, Math.max(p().total, 1))} of {Math.max(p().total, 1)}
                {p().filesTotal > 0 ? ` · ${p().filesDone.toLocaleString()} / ${p().filesTotal.toLocaleString()} files` : ""}
              </>
            )}
          </Show>
          </Show>
        }
        sub={
          <>
            <Show when={library.scanError()}>
              <p class="warn">Scan failed: {library.scanError()}</p>
            </Show>
            <Show when={library.lastScan()?.warnings.length}>
              <details>
                <summary>{library.lastScan()!.warnings.length} skipped / warnings</summary>
                <ul class="warnings">
                  <For each={library.lastScan()!.warnings}>{(w) => <li class="mono">{w}</li>}</For>
                </ul>
              </details>
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
          </>
        }
      >
        <Show
          when={library.scanProgress()}
          fallback={
            <button class="btn primary" onClick={() => void library.scan()}>
              Scan library
            </button>
          }
        >
          <button class="btn" onClick={() => void api.cancelScan()}>
            Cancel scan
          </button>
        </Show>
      </Row>

      <Row
        k="covers"
        status={
          <Show
            when={library.artProgress()}
            fallback={`${library.games().length - coverless().length} of ${library.games().length} have a cover`}
          >
            {(p) => `Fetching: ${p().done} / ${p().total} · ${p().found} found`}
          </Show>
        }
        open={!library.artProgress() && coverless().length > 0 && "Missing…"}
        sub={<MissedGames games={coverless()} label="No cover yet" />}
        toggle={
          <OnOff
            label="Fetch covers automatically after each scan"
            checked={props.s.values.autoArt !== false}
            onChange={(on) => void props.run(() => api.setSetting("autoArt", on))}
          />
        }
      >
        <Show
          when={library.artProgress()}
          fallback={
            <button class="btn" disabled={!!library.scanProgress()} onClick={() => void library.fetchArt()}>
              Fetch missing
            </button>
          }
        >
          <button class="btn" onClick={() => void api.cancelScan()}>
            Stop
          </button>
        </Show>
      </Row>

      <Row
        k="folders"
        status={`${props.s.sources.length} folder${props.s.sources.length === 1 ? "" : "s"}${props.s.sources.some((x) => !x.exists) ? " · some not found" : ""}`}
        open="Edit…"
        sub={
          <>
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
            <Show when={!props.s.sources.some((x) => x.kind === "ps3_hdd0")}>
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
                  Add RPCS3 installed games
                </button>
              </div>
            </Show>
          </>
        }
      />
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

/** Whether an emulator earns RetroAchievements (from RA's list of supported emulators). */
function achievementsNote(a: boolean | null): string {
  return a === true ? "Achievements" : a === false ? "No achievements" : "";
}

function EmulatorsSection(props: { s: Settings; run: Run }) {
  const [browsing, setBrowsing] = createSignal(false);
  const coresDir = () => (props.s.values.retroarchCoresDir as string | undefined) ?? "";
  const builtIn = () => props.s.emulators.filter((e) => !e.custom);

  const detect = (folder: string) =>
    props.run(
      async () => {
        const found = await api.detectEmulators(folder);
        if (found.length === 0) throw "No known emulator executables found in that folder.";
      },
      { notice: "Emulator paths filled in from the folder." },
    );

  return (
    <section class="card-section set-list">
      <Row k="findEmus" status={`${builtIn().filter((e) => e.exeExists).length} of ${builtIn().length} set up`}>
        <button class="btn primary" onClick={() => setBrowsing(true)}>
          Find emulators…
        </button>
      </Row>
      <div class="set-tiles">
        <For each={builtIn()}>{(emu) => <EmulatorEditor emu={emu} run={props.run} cores={coresDir()} />}</For>
      </div>
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

function EmulatorEditor(props: { emu: EmulatorRow; run: Run; cores?: string }) {
  const save = (exe: string | null, tpl: string | null) => props.run(() => api.updateEmulator(props.emu.id, exe, tpl));
  return (
    <Row
      tile
      dot={props.emu.exeExists ? "ok" : props.emu.exePath ? "warn" : "off"}
      words={`emulator exe path template ${props.emu.exeExists ? "found" : props.emu.exePath ? "file missing" : "not set"}`}
      k={`emu-${props.emu.id}`}
      title={props.emu.name}
      detail={`Where ${props.emu.name}'s .exe is, and how KB starts it. Leave the launch template empty for the default (shown in grey); placeholders: {emulator} {core} {rom} {rom_dir} {rom_name}.${achievementsNote(props.emu.achievements) ? ` ${achievementsNote(props.emu.achievements)} with RetroAchievements.` : ""}`}
      status={
        <span class="status" classList={{ ok: props.emu.exeExists }}>
          {props.emu.exeExists ? "found" : props.emu.exePath ? "file missing" : "not set"}
          <Show when={achievementsNote(props.emu.achievements)}>{` · ${achievementsNote(props.emu.achievements)}`}</Show>
        </span>
      }
      sub={
        <div class="set-fields">
          <label class="field-label">Executable</label>
          <PathField
            label={`${props.emu.name} executable`}
            value={props.emu.exePath ?? ""}
            placeholder="Path to the emulator .exe"
            mode="file"
            onCommit={(v) => save(v || null, props.emu.templateOverride)}
          />
          <label class="field-label">Launch template (empty = the default in grey)</label>
          <input
            class="input mono"
            aria-label={`${props.emu.name} launch template`}
            value={props.emu.templateOverride ?? ""}
            placeholder={props.emu.defaultTemplate}
            onChange={(e) => save(props.emu.exePath, e.currentTarget.value || null)}
          />
          <Show when={props.emu.id === "retroarch"}>
            <label class="field-label">RetroArch cores folder</label>
            <PathField
              label="RetroArch cores folder"
              value={props.cores ?? ""}
              placeholder="e.g. E:\Emulators\RetroArch\cores"
              mode="folder"
              onCommit={(v) => props.run(() => api.setSetting("retroarchCoresDir", v))}
            />
            <p class="dim small">
              RetroArch's cores (its .dll files) are kept in this folder. Every platform played with RetroArch loads its core from here, so those games won't start without it.
            </p>
          </Show>
        </div>
      }
    />
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
  // The platform whose emulators panel is open.
  const [emusOf, setEmusOf] = createSignal<string | null>(null);
  const defaultOf = (p: PlatformRow) => p.emulatorChoice ?? p.emulatorId;
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
    <section class="card-section set-list" id="set-platforms">
      <div class="set-tiles">
      <For each={props.s.platforms}>
        {(p) => (
          <Row
            tile
            dot={p.coreProblem ? "warn" : p.enabled ? "ok" : "off"}
            words={`${p.shortName} platform core extensions emulators ${emuName(defaultOf(p))} ${p.enabled ? "on" : "off"} ${p.coreProblem ?? ""}`}
            toggleLabel={`Use ${p.name}`}
            label={p.shortName}
            k={`plat-${p.id}`}
            title={p.name}
            detail={
              p.coreProblem ??
              `${p.name}: ${p.gameCount} game${p.gameCount === 1 ? "" : "s"}. Edit… chooses its emulators and default, and overrides the core, launch template and file types (empty fields use the defaults in grey). The switch turns the platform on or off.`
            }
            status={
              <Show when={!p.coreProblem} fallback={<span class="warn">{p.coreProblem}</span>}>
                {emuName(defaultOf(p))}
                {p.extraEmulators.length > 0 ? ` +${p.extraEmulators.length}` : ""}
                {p.emulatorId === "retroarch" ? ` · ${p.coreOverride ?? p.defaultCore ?? ""}` : ""} · {p.gameCount} game
                {p.gameCount === 1 ? "" : "s"}
              </Show>
            }
            sub={
              <>
                <div class="form-grid">
                  <label>Emulators</label>
                  <button class="btn" aria-label={`${p.name} emulators`} onClick={() => setEmusOf(p.id)}>
                    {emuName(defaultOf(p))}
                    {p.extraEmulators.length > 0 ? ` +${p.extraEmulators.length}` : ""} ▾
                  </button>
                  <Show when={p.emulatorId === "retroarch"}>
                    <label for={`core-${p.id}`}>Core</label>
                    <input
                      id={`core-${p.id}`}
                      class="input mono"
                      value={p.coreOverride ?? ""}
                      placeholder={p.defaultCore ?? ""}
                      onChange={(e) => save(p, { coreOverride: e.currentTarget.value.trim() || null })}
                    />
                  </Show>
                  <label for={`tpl-${p.id}`}>Launch template</label>
                  <input
                    id={`tpl-${p.id}`}
                    class="input mono"
                    value={p.templateOverride ?? ""}
                    placeholder={emuTemplate(p.emulatorId)}
                    onChange={(e) => save(p, { templateOverride: e.currentTarget.value.trim() || null })}
                  />
                  <label for={`ext-${p.id}`}>File types</label>
                  <Show when={p.id !== "ps3"} fallback={<span class="dim">folders (PS3_GAME)</span>}>
                    <input
                      id={`ext-${p.id}`}
                      class="input mono"
                      value={p.extensionsOverride?.join(", ") ?? ""}
                      placeholder={p.defaultExtensions.join(", ")}
                      onChange={(e) => save(p, { extensionsOverride: parseExts(e.currentTarget.value) })}
                    />
                  </Show>
                </div>
                <p class="dim small">{INFO.platforms.detail}</p>
              </>
            }
            toggle={<OnOff label={`Use ${p.name}`} checked={p.enabled} onChange={(on) => void save(p, { enabled: on })} />}
          />
        )}
      </For>
      </div>
      <Show when={props.s.platforms.find((p) => p.id === emusOf())}>
        {(p) => <PlatformEmulatorsPanel p={p()} s={props.s} run={props.run} onClose={() => setEmusOf(null)} />}
      </Show>
    </section>
  );
}

/**
 * A platform's emulators: the built-in one and those added, which one is the
 * default, each added one's core and template, and adding or removing them.
 */
function PlatformEmulatorsPanel(props: { p: PlatformRow; s: Settings; run: Run; onClose: () => void }) {
  const emu = (id: string) => props.s.emulators.find((e) => e.id === id);
  const defaultId = () => props.p.emulatorChoice ?? props.p.emulatorId;
  const rows = () => [
    {
      emulatorId: props.p.emulatorId,
      core: null as string | null,
      templateOverride: null as string | null,
      coreProblem: props.p.coreProblem,
      builtIn: true,
    },
    ...props.p.extraEmulators.map((e) => ({ ...e, builtIn: false })),
  ];
  return (
    <div class="overlay" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <NavScope class="panel platform-emus-panel" modal onBack={props.onClose}>
        <h2>{props.p.name}: emulators</h2>
        <p class="dim small">Games use the default unless you pick another with "Launch with" on the game.</p>
        <For each={rows()}>
          {(r) => {
            const e = () => emu(r.emulatorId);
            const isDefault = () => r.emulatorId === defaultId();
            return (
              <div class="emu platform-emu">
                <h3>
                  <EmulatorBadge id={r.emulatorId} name={e()?.name} />
                  {e()?.name ?? r.emulatorId}
                  <span class="status" classList={{ ok: !!e()?.exeExists }}>
                    {e()?.exeExists ? "found" : e()?.exePath ? "file missing" : "not set"}
                  </span>
                  <Show when={achievementsNote(e()?.achievements ?? null)}>
                    <span class="dim small"> · {achievementsNote(e()?.achievements ?? null)}</span>
                  </Show>
                  <Show when={r.builtIn}>
                    <span class="dim small"> · built-in</span>
                  </Show>
                </h3>
                <Show when={r.coreProblem}>
                  <p class="warn small">{r.coreProblem}</p>
                </Show>
                <div class="row">
                  <Show
                    when={isDefault()}
                    fallback={
                      <button class="btn small" onClick={() => void props.run(() => api.setPlatformDefault(props.p.id, r.emulatorId))}>
                        Make default
                      </button>
                    }
                  >
                    <span class="badge">Default</span>
                  </Show>
                  <Show when={!r.builtIn}>
                    <button
                      class="btn small"
                      onClick={() => void props.run(() => api.removePlatformEmulator(props.p.id, r.emulatorId))}
                    >
                      Remove from {props.p.shortName}
                    </button>
                  </Show>
                </div>
                <Show when={!r.builtIn}>
                  <Show when={r.emulatorId === "retroarch"}>
                    <label class="field-label">Core</label>
                    <input
                      class="input mono"
                      aria-label={`RetroArch core for ${props.p.name}`}
                      value={r.core ?? ""}
                      placeholder="core name, e.g. swanstation"
                      onChange={(ev) =>
                        void props.run(() =>
                          api.updatePlatformEmulator(props.p.id, r.emulatorId, ev.currentTarget.value.trim() || null, r.templateOverride),
                        )
                      }
                    />
                  </Show>
                  <label class="field-label">Launch template</label>
                  <input
                    class="input mono"
                    aria-label={`${e()?.name ?? r.emulatorId} launch template for ${props.p.name}`}
                    value={r.templateOverride ?? ""}
                    placeholder={e()?.templateOverride ?? e()?.defaultTemplate ?? ""}
                    onChange={(ev) =>
                      void props.run(() =>
                        api.updatePlatformEmulator(props.p.id, r.emulatorId, r.core, ev.currentTarget.value.trim() || null),
                      )
                    }
                  />
                </Show>
              </div>
            );
          }}
        </For>
        <Show
          when={props.p.addableEmulators.length > 0}
          fallback={<p class="dim small">To add another emulator, first add it under Settings → Emulators → Add emulator….</p>}
        >
          <div class="row">
            <Picker
              title={`Add an emulator to ${props.p.name}`}
              label="Add an emulator…"
              value=""
              options={props.p.addableEmulators.map((id) => ({ value: id, label: emu(id)?.name ?? id }))}
              onChange={(id) => void props.run(() => api.addPlatformEmulator(props.p.id, id))}
            />
          </div>
        </Show>
        <div class="panel-actions">
          <div class="hints-row">
            <Hint action="select" label="Choose" />
            <Hint action="back" label="Close" />
          </div>
          <button class="btn" data-autofocus onClick={props.onClose}>
            Close
          </button>
        </div>
      </NavScope>
    </div>
  );
}

// ------------------------------------------------------------------ general

// ------------------------------------------------------------------ services

function LaunchBoxSection(props: { run: Run }) {
  const [status, { refetch }] = createResource(() => api.lbStatus().catch(() => null));
  // Refresh the counts when a pass finishes.
  createEffect(on(library.lastLb, () => void refetch(), { defer: true }));
  const when = (secs: number) => new Date(secs * 1000).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });

  return (
    <section class="card-section set-list">
      <Show when={status()}>
        {(st) => (
          <Row
            k="launchbox"
            status={
              library.lbError() ? (
                <span class="warn">{library.lbError()}</span>
              ) : !st().enabled ? (
                "Off"
              ) : st().total > 0 ? (
                `${st().matched} of ${st().total} found${st().importedAt ? ` · updated ${when(st().importedAt!)}` : ""}`
              ) : (
                "No games on the platforms it covers yet"
              )
            }
            open={st().enabled && st().unmatched.length > 0 && "Missing…"}
            sub={<MissedGames games={st().unmatched} />}
            toggle={
              <OnOff
                label="Use the LaunchBox Games Database"
                checked={st().enabled}
                onChange={(on) =>
                  void props.run(async () => {
                    await api.setLbEnabled(on);
                    await refetch();
                  })
                }
              />
            }
          >
            <Show when={st().enabled}>
              <Show
                when={library.lbProgress()}
                fallback={
                  <button
                    class="btn"
                    disabled={!!library.scanProgress()}
                    onClick={() => void props.run(() => api.fetchLaunchbox(true), { notice: "Updating from LaunchBox…" })}
                  >
                    Update now
                  </button>
                }
              >
                <button class="btn" onClick={() => void api.cancelScan()}>
                  Stop
                </button>
              </Show>
            </Show>
          </Row>
        )}
      </Show>
    </section>
  );
}

function WikidataSection(props: { run: Run }) {
  const [status, { refetch }] = createResource(() => api.wdStatus().catch(() => null));
  createEffect(on(library.lastWd, () => void refetch(), { defer: true }));
  const when = (secs: number) => new Date(secs * 1000).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });

  return (
    <section class="card-section set-list">
      <Show when={status()}>
        {(st) => (
          <Row
            k="wikidata"
            status={
              library.wdError() ? (
                <span class="warn">{library.wdError()}</span>
              ) : !st().enabled ? (
                "Off"
              ) : st().total > 0 ? (
                `${st().withSeries} in a series · ${st().matched} of ${st().total} found${st().checkedAt ? ` · asked ${when(st().checkedAt!)}` : ""}`
              ) : (
                "No games yet"
              )
            }
            open={st().enabled && st().unmatched.length > 0 && "Missing…"}
            sub={<MissedGames games={st().unmatched} />}
            toggle={
              <OnOff
                label="Use Wikidata"
                checked={st().enabled}
                onChange={(on) =>
                  void props.run(async () => {
                    await api.setWdEnabled(on);
                    await refetch();
                  })
                }
              />
            }
          >
            <Show when={st().enabled}>
              <Show
                when={library.wdProgress()}
                fallback={
                  <button
                    class="btn"
                    disabled={!!library.scanProgress()}
                    onClick={() => void props.run(() => api.fetchWikidata(true), { notice: "Asking Wikidata…" })}
                  >
                    Update now
                  </button>
                }
              >
                <button class="btn" onClick={() => void api.cancelScan()}>
                  Stop
                </button>
              </Show>
            </Show>
          </Row>
        )}
      </Show>
    </section>
  );
}

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
    <section class="card-section set-list">
      <Show when={services()}>
        {(sv) => (
          <Show
            when={sv().igdb}
            fallback={
              <Row k="igdb" status="Not set up · optional key" open="Set up…" sub={<>
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
              </>} />
            }
          >
            <Row
              k="igdb"
              status={
                <Show
                  when={library.metaProgress()}
                  fallback={
                    <>
                      <Show when={library.metaError()} fallback={<span class="ok-text">Connected</span>}>
                        <span class="warn">{library.metaError()}</span>
                      </Show>
                      <Show when={library.lastMeta()}>{(m) => ` · last run ${m().matched} found, ${m().notFound} not found`}</Show>
                    </>
                  }
                >
                  {(p) => `Looking up: ${p().done} / ${p().total} · ${p().matched} found`}
                </Show>
              }
              open="Key…"
              sub={
                <div class="row">
                  <span>
                    <span class="ok-text">Connected</span> with Client ID <span class="mono">{sv().igdbClientId}</span>
                  </span>
                  <button class="btn danger" onClick={() => void remove()}>
                    Remove keys
                  </button>
                </div>
              }
              toggle={
                <OnOff
                  label="Look up new games automatically after each scan"
                  checked={props.s.values.autoMeta !== false}
                  onChange={(on) => void props.run(() => api.setSetting("autoMeta", on))}
                />
              }
            >
              <Show
                when={library.metaProgress()}
                fallback={
                  <button class="btn" disabled={!!library.scanProgress()} onClick={() => void api.fetchMetadata()}>
                    Look up
                  </button>
                }
              >
                <button class="btn" onClick={() => void api.cancelScan()}>
                  Stop
                </button>
              </Show>
            </Row>
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
    <section class="card-section set-list">
      <Show when={services()}>
        {(sv) => (
          <Show
            when={sv().sgdb}
            fallback={
              <Row k="sgdb" status="Not set up · optional key" open="Set up…" sub={<>
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
              </>} />
            }
          >
            <Row
              k="sgdb"
              status={
                <Show
                  when={library.sgdbProgress()}
                  fallback={
                    <>
                      <Show when={library.sgdbError()} fallback={<span class="ok-text">Connected</span>}>
                        <span class="warn">{library.sgdbError()}</span>
                      </Show>
                      <Show when={library.lastSgdb()}>
                        {(s) => ` · last run ${s().heroes} backgrounds, ${s().logos} logos, ${s().covers} covers`}
                      </Show>
                    </>
                  }
                >
                  {(p) => `Looking up: ${p().done} / ${p().total} · ${p().matched} found`}
                </Show>
              }
              open="Key…"
              sub={
                <div class="row">
                  <span class="ok-text">Connected</span>
                  <button class="btn danger" onClick={() => void remove()}>
                    Remove key
                  </button>
                </div>
              }
            >
              <Show
                when={library.sgdbProgress()}
                fallback={
                  <button class="btn" disabled={!!library.scanProgress()} onClick={() => void api.fetchExtras()}>
                    Look up
                  </button>
                }
              >
                <button class="btn" onClick={() => void api.cancelScan()}>
                  Stop
                </button>
              </Show>
            </Row>
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
    <section class="card-section set-list">
      <Show when={services()}>
        {(sv) => (
          <Show
            when={sv().raUser}
            fallback={
              <Row k="ra" status="Not set up · optional key" open="Set up…" sub={<>
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
              </>} />
            }
          >
            <Row
              k="ra"
              detail={`${INFO.ra.detail} "Not recognized" means RetroAchievements doesn't list that exact dump (a hack, a translation or another revision).`}
              status={
                <Show
                  when={library.raProgress()}
                  fallback={
                    <>
                      <Show when={library.raError()} fallback={<span class="ok-text">Connected</span>}>
                        <span class="warn">{library.raError()}</span>
                      </Show>{" "}
                      as {sv().raUser}
                      <Show when={library.lastRa()}>
                        {(s) => ` · ${s().matched} recognized${s().unknown > 0 ? `, ${s().unknown} not` : ""}`}
                      </Show>
                    </>
                  }
                >
                  {(p) => `Identifying: ${p().done} / ${p().total} · ${p().matched} recognized`}
                </Show>
              }
              open="Key…"
              sub={
                <div class="row">
                  <span>
                    <span class="ok-text">Connected</span> as {sv().raUser}
                  </span>
                  <button class="btn danger" onClick={() => void remove()}>
                    Remove key
                  </button>
                </div>
              }
            >
              <Show when={library.raProgress()}>
                <button class="btn" onClick={() => void api.cancelScan()}>
                  Stop
                </button>
              </Show>
            </Row>
          </Show>
        )}
      </Show>
    </section>
  );
}

/** The exit combo: which buttons close the running game, and for how long to hold them. */
function ControllerRows(props: { s: Settings; run: Run }) {
  const combo = (): ExitCombo =>
    (props.s.values.exitCombo as ExitCombo | undefined) ?? { buttons: ["back", "start"], holdMs: 1000 };
  const family = () => session.lastPad()?.family;
  const toggle = (b: string, on: boolean) => {
    const buttons = on ? [...combo().buttons, b] : combo().buttons.filter((x) => x !== b);
    return props.run(() => api.setSetting("exitCombo", { ...combo(), buttons }));
  };
  return (
    <Row
      k="exitCombo"
      detail={`${INFO.exitCombo.detail}${session.lastPad() ? ` Labels shown for: ${session.lastPad()!.name}.` : ""}`}
      status={`Hold ${combo().buttons.map((b) => buttonLabel(b, family())).join(" + ")} for ${combo().holdMs / 1000} s`}
      open="Change…"
      sub={
        <>
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
          <div class="form-grid">
            <label>Hold for</label>
            <Picker
              title="Hold the combo for"
              value={String(combo().holdMs)}
              options={[500, 1000, 1500, 2000, 3000].map((ms) => ({ value: String(ms), label: `${ms / 1000} s` }))}
              onChange={(v) => props.run(() => api.setSetting("exitCombo", { ...combo(), holdMs: Number(v) }))}
            />
          </div>
        </>
      }
    />
  );
}

/** Look and start-up, motion and sound: the groups of the right-hand column. */
function GeneralSection(props: { s: Settings; run: Run }) {
  const [themes, { mutate }] = createResource(() => api.listThemes());
  const current = () => themes()?.[0].find((t) => t.id === themes()?.[1]);
  // "Start fullscreen" and "Start in TV Mode" are one choice: only one of them is ever on.
  const opensIn = () => (props.s.values.startInTv === true ? "tv" : props.s.values.tvMode === true ? "fullscreen" : "window");
  const setOpensIn = (v: string) =>
    props.run(async () => {
      await api.setSetting("tvMode", v === "fullscreen");
      await api.setSetting("startInTv", v === "tv");
    });
  return (
    <>
      <Group title="Look and start-up">
        <Row k="style" status={current() ? `${current()!.name} · ${FOCUS_LOOKS[current()!.focusLook] ?? ""} focus` : ""}>
          <Show when={themes()}>
            {(list) => (
              <Picker
                dropdown title="Style"
                value={list()[1]}
                options={list()[0].map((t) => ({ value: t.id, label: t.name }))}
                onChange={(id) =>
                  props.run(async () => {
                    applyTheme(await api.setTheme(id));
                    mutate([list()[0], id]);
                  })
                }
              />
            )}
          </Show>
        </Row>
        <Row k="size">
          <Picker
            dropdown title="Interface size"
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
        </Row>
        <Row k="opensIn">
          <Picker
            dropdown title="KB opens in"
            value={opensIn()}
            options={[
              { value: "window", label: "A window" },
              { value: "fullscreen", label: "Fullscreen" },
              { value: "tv", label: "TV Mode" },
            ]}
            onChange={setOpensIn}
          />
        </Row>
        <IntroRows />
      </Group>
      <Group title="Motion">
        <Row k="animDesktop">
          <Picker dropdown title="Desktop animations" value={fx.get("animDesktop")} options={LEVELS} onChange={(v) => fx.set("animDesktop", v)} />
        </Row>
        <Row k="animTv">
          <Picker dropdown title="TV Mode animations" value={fx.get("animTv")} options={LEVELS} onChange={(v) => fx.set("animTv", v)} />
        </Row>
        <Row
          k="launchFx"
          toggle={
            <OnOff
              label="Launch animation"
              checked={fx.get("launchFxDesktop") || fx.get("launchFxTv")}
              onChange={(on) => {
                fx.set("launchFxDesktop", on);
                fx.set("launchFxTv", on);
              }}
            />
          }
        />
      </Group>
      <Group title="Sound">
        <SoundRows />
      </Group>
    </>
  );
}
const size = (n: number) =>
  n >= 1073741824 ? `${(n / 1073741824).toFixed(1)} GB` : n >= 1048576 ? `${(n / 1048576).toFixed(0)} MB` : `${Math.round(n / 1024)} KB`;

function StorageBlock(props: { s: Settings; run: Run }) {
  const [storage, { refetch }] = createResource(() => api.getStorage().catch(() => null));
  const [removed, { refetch: refetchRemoved }] = createResource(() => api.getRemoved().catch(() => null));
  const [asking, setAsking] = createSignal<"clear" | "reset" | "forget" | "backup" | null>(null);
  const [forgetKeys, setForgetKeys] = createSignal(false);
  const [working, setWorking] = createSignal(false);
  const [failed, setFailed] = createSignal<string | null>(null);

  const ask = (what: "clear" | "reset" | "forget" | "backup") => {
    setForgetKeys(false);
    setFailed(null);
    setAsking(what);
  };
  const close = () => !working() && setAsking(null);
  const clearWebCache = () =>
    props.run(
      async () => {
        await api.clearWebCache();
        await refetch();
      },
      { notice: "Web cache cleared." },
    );

  async function confirm() {
    setWorking(true);
    setFailed(null);
    try {
      if (asking() === "reset") {
        await api.resetApp(forgetKeys());
        location.reload();
        return;
      }
      if (asking() === "forget") {
        const n = await api.forgetRemoved();
        setAsking(null);
        await props.run(() => library.reload(), { notice: `Forgot ${n} removed game${n === 1 ? "" : "s"}.` });
        await refetchRemoved();
        await refetch();
        return;
      }
      if (asking() === "backup") {
        await api.deleteResetBackup();
        setAsking(null);
        await refetch();
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
      <Row
        k="storage"
        detail={`${INFO.storage.detail}${storage() ? ` KB's folder: ${storage()!.dataDir}.` : ""}`}
        status={`Downloaded ${storage() ? size(storage()!.downloadedBytes) : "…"} · web cache ${storage() ? size(storage()!.webCacheBytes) : "…"}`}
        open="Manage…"
        sub={
          <>
            <Show when={storage()}>
              {(st) => (
                <table class="table">
                  <tbody>
                    <tr>
                      <td>Downloaded covers, pictures, game info and achievement data</td>
                      <td class="num">{size(st().downloadedBytes)}</td>
                    </tr>
                    <tr>
                      <td>Web cache (picker previews)</td>
                      <td class="num">{size(st().webCacheBytes)}</td>
                    </tr>
                    <Show when={st().launchboxBytes}>
                      <tr>
                        <td>LaunchBox game database (kept so new games need no download)</td>
                        <td class="num">{size(st().launchboxBytes!)}</td>
                      </tr>
                    </Show>
                    <Show when={st().resetBackupBytes}>
                      <tr>
                        <td>Reset backup (library.before-reset.db)</td>
                        <td class="num">{size(st().resetBackupBytes!)}</td>
                      </tr>
                    </Show>
                  </tbody>
                </table>
              )}
            </Show>
            <div class="row wrap">
              <button class="btn" data-autofocus onClick={() => void clearWebCache()}>
                Clear web cache
              </button>
              <button class="btn" onClick={() => ask("clear")}>
                Clear downloaded data…
              </button>
              <Show when={storage()?.launchboxBytes}>
                <button
                  class="btn"
                  onClick={() =>
                    void props.run(
                      async () => {
                        await api.deleteLaunchboxFile();
                        await refetch();
                      },
                      { notice: "LaunchBox file deleted. It downloads again (about 100 MB) the next time it's needed." },
                    )
                  }
                >
                  Delete LaunchBox file
                </button>
              </Show>
              <Show when={storage()?.resetBackupBytes}>
                <button class="btn" onClick={() => ask("backup")}>
                  Delete reset backup…
                </button>
              </Show>
              <button class="btn danger" onClick={() => ask("reset")}>
                Reset KaneegaBell…
              </button>
            </div>
            <p class="dim small">
              Clear web cache only empties KB's page cache; it refills as needed and your covers and game info stay.
            </p>
          </>
        }
      />
      <Row
        k="removed"
        status={removed()?.count ? `${removed()!.count} hidden (file gone)` : "None kept"}
        toggle={
          <OnOff
            label="Forget removed games automatically"
            checked={props.s.values.forgetRemovedGames === true}
            onChange={(on) => void props.run(() => api.setSetting("forgetRemovedGames", on))}
          />
        }
      >
        <button class="btn" disabled={!removed()?.count} onClick={() => ask("forget")}>
          Forget them…
        </button>
      </Row>
      <Show when={asking()}>
        {(what) => (
          <div class="overlay" onClick={(e) => e.target === e.currentTarget && close()}>
            <NavScope class="panel confirm-panel" modal onBack={close}>
              <Show when={what() === "forget"}>
                <h2>Forget removed games?</h2>
                <p>
                  KB deletes the {removed()?.count ?? ""} hidden game{removed()?.count === 1 ? "" : "s"} whose files are
                  gone, with their play time, favorites, collection places and pictures. This can't be undone.
                </p>
                <p class="dim">If a file comes back later, its game is found again by the next scan, as a new game.</p>
              </Show>
              <Show when={what() === "backup"}>
                <h2>Delete the reset backup?</h2>
                <p>
                  KB deletes <span class="mono">library.before-reset.db</span>, the copy of your library from before the
                  last reset. Your current library isn't affected. This can't be undone.
                </p>
              </Show>
              <Show
                when={what() !== "forget" && what() !== "backup"}
              >
              <Show
                when={what() === "reset"}
                fallback={
                  <>
                    <h2>Clear downloaded data?</h2>
                    <p>
                      KB deletes the covers, backgrounds, logos, game info and achievement data it downloaded, then
                      downloads them again in the background. Use this if artwork or info looks wrong or out of date.
                      The web cache (picker previews) is emptied the next time KB starts.
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
              </Show>
              <Show when={failed()}>
                <p class="warn">{failed()}</p>
              </Show>
              <div class="row">
                <button class="btn" data-autofocus disabled={working()} onClick={close}>
                  Cancel
                </button>
                <button class="btn danger" disabled={working()} onClick={() => void confirm()}>
                  {working()
                    ? "Working…"
                    : what() === "reset"
                      ? "Reset KaneegaBell"
                      : what() === "forget"
                        ? "Forget them"
                        : what() === "backup"
                          ? "Delete it"
                          : "Clear downloaded data"}
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

const LEVELS: { value: AnimLevel; label: string }[] = [
  { value: "full", label: "Full" },
  { value: "reduced", label: "Reduced" },
  { value: "off", label: "Off" },
];

const ONE_INTRO = (Object.entries(INTROS) as [IntroKey, { name: string }][]).map(([value, i]) => ({ value, label: i.name }));
const INTRO_PICKS: { value: IntroPick; label: string }[] = [{ value: "random", label: "Random" }, ...ONE_INTRO, { value: "off", label: "Off" }];

/** One start-up intro for the desktop and TV Mode alike, and a place to watch them. */
function IntroRows() {
  const [preview, setPreview] = createSignal<IntroKey>("konami");
  return (
    <>
      <Row k="intro">
        <Picker
          dropdown title="Start-up intro"
          value={fx.get("introDesktop")}
          options={INTRO_PICKS}
          onChange={(v) => {
            fx.set("introDesktop", v);
            fx.set("introTv", v);
          }}
        />
      </Row>
      <Row k="introWatch">
        <Picker dropdown title="Intro to watch" value={preview()} options={ONE_INTRO} onChange={setPreview} />
        <button class="btn" onClick={() => playIntro(preview())}>
          Play
        </button>
      </Row>
    </>
  );
}

const PROFILES: { value: SoundProfile; label: string }[] = [
  { value: "tactile", label: "Tactile" },
  { value: "console", label: "Console" },
  { value: "retro", label: "Retro console" },
];
/** Off, or how loud: one choice instead of a switch and a volume. */
const SOUND_LEVELS = [
  { value: "off", label: "Off" },
  ...Array.from({ length: 10 }, (_, i) => ({ value: String((i + 1) * 10), label: `${(i + 1) * 10}%` })),
];

function SoundRows() {
  const level = (on: boolean, volume: number) => (on ? String(volume) : "off");
  const setLevel = (onKey: "soundDesktop" | "soundTv", volumeKey: "volumeDesktop" | "volumeTv") => (v: string) => {
    fx.set(onKey, v !== "off");
    if (v === "off") return;
    fx.set(volumeKey, Number(v));
    sfx("select");
  };
  return (
    <>
      <Row k="soundProfile">
        <Picker
          dropdown title="Sound profile"
          value={fx.get("soundProfile")}
          options={PROFILES}
          onChange={(v) => {
            fx.set("soundProfile", v);
            sfx("select");
          }}
        />
      </Row>
      <Row k="soundDesktop">
        <Picker dropdown title="Desktop sounds" value={level(fx.get("soundDesktop"), fx.get("volumeDesktop"))} options={SOUND_LEVELS} onChange={setLevel("soundDesktop", "volumeDesktop")} />
      </Row>
      <Row k="soundTv">
        <Picker dropdown title="TV Mode sounds" value={level(fx.get("soundTv"), fx.get("volumeTv"))} options={SOUND_LEVELS} onChange={setLevel("soundTv", "volumeTv")} />
      </Row>
    </>
  );
}

function TvRows(props: { s: Settings; run: Run }) {
  return (
    <>
      <Row k="tvText">
        <Picker
          title="TV Mode text size"
          value={(props.s.values.tvTextSize as string | undefined) ?? "m"}
          options={[
            { value: "s", label: "Small" },
            { value: "m", label: "Medium" },
            { value: "l", label: "Large" },
            { value: "xl", label: "Extra large" },
          ]}
          onChange={(v) => props.run(() => api.setSetting("tvTextSize", v))}
        />
      </Row>
      <Row k="tvShelf">
        <Picker
          title="TV Mode case"
          value={fx.get("tvShelf")}
          options={[
            { value: "3d", label: "3D case" },
            { value: "flat", label: "Flat cover" },
          ]}
          onChange={(v) => fx.set("tvShelf", v)}
        />
      </Row>
    </>
  );
}

function UpdatesBlock(props: { s: Settings; run: Run }) {
  const [info] = createResource(() => api.appInfo().catch(() => null));
  const [checking, setChecking] = createSignal(false);
  const [result, setResult] = createSignal<string | null>(null);
  const [failed, setFailed] = createSignal(false);
  const [hasCode, { refetch: refetchCode }] = createResource(() => api.hasUpdateCode().catch(() => false));

  // One button: a new version opens the update dialog (with its notes), otherwise What's new.
  async function checkNow() {
    setChecking(true);
    setResult(null);
    setFailed(false);
    try {
      if (!(await updates.checkNow())) {
        setResult("You have the latest version.");
        whatsNew.setOpen(true);
      }
    } catch (e) {
      setResult(errorText(e));
      setFailed(true);
    } finally {
      setChecking(false);
    }
  }

  return (
    <Row
      k="updates"
      detail={`${INFO.updates.detail}${info()?.devBuild ? " This copy runs from a build folder, so it doesn't check by itself; use Check now." : ""} The switch checks once when KaneegaBell starts.`}
      status={
        <span classList={{ warn: failed() }}>
          {result() ?? `Version ${info()?.version ?? ""}${props.s.values.autoUpdateCheck !== false ? " · checks at start" : ""}`}
        </span>
      }
      // The access code only matters until it's saved, or when it stops working.
      open={(failed() || hasCode() === false) && "Access code…"}
      sub={
        <UpdateCodeField
          run={props.run}
          onSaved={() => {
            void refetchCode();
            void checkNow();
          }}
        />
      }
      toggle={
        <OnOff
          label="Check for updates when KaneegaBell starts"
          checked={props.s.values.autoUpdateCheck !== false}
          onChange={(on) => void props.run(() => api.setSetting("autoUpdateCheck", on))}
        />
      }
    >
      <button class="btn" disabled={checking()} onClick={() => void checkNow()}>
        {checking() ? "Checking…" : "Check now"}
      </button>
    </Row>
  );
}

/** The update access code: lets this copy read KaneegaBell's private releases. */
function UpdateCodeField(props: { run: Run; onSaved: () => void }) {
  const [has, { refetch }] = createResource(() => api.hasUpdateCode().catch(() => false));
  const [code, setCode] = createSignal("");
  const save = () =>
    props.run(
      async () => {
        await api.setUpdateCode(code());
        setCode("");
        await refetch();
        props.onSaved();
      },
      { notice: "Access code saved. This copy can get updates now." },
    );
  const remove = () =>
    props.run(async () => {
      await api.removeUpdateCode();
      await refetch();
    });

  return (
    <>
      <label class="field-label">Update access code</label>
      <Show
        when={has()}
        fallback={
          <>
            <div class="row">
              <input
                class="input grow mono"
                type="password"
                aria-label="Update access code"
                autocomplete="off"
                placeholder="Paste the code you were given"
                value={code()}
                onInput={(e) => setCode(e.currentTarget.value)}
                onKeyDown={(e) => e.key === "Enter" && code().trim() && void save()}
              />
              <button class="btn primary" disabled={!code().trim()} onClick={() => void save()}>
                Save
              </button>
            </div>
            <p class="dim small">
              KaneegaBell's updates are private. Ask the person who gave you KaneegaBell for an access code, paste it here
              once, and updates arrive by themselves. It's kept in KB's folder, encrypted with your Windows login.
            </p>
          </>
        }
      >
        <div class="row">
          <span class="ok-text">Saved</span>
          <button class="btn small danger" onClick={() => void remove()}>
            Remove code
          </button>
        </div>
        <p class="dim small">
          If updates stop with an "expired" or "doesn't work" message, ask for a new code and enter it here. (Making
          codes: a GitHub fine-grained token for only the KaneegaBell repository, with Contents: Read-only; see the
          README.)
        </p>
      </Show>
    </>
  );
}
