import { createResource, createSignal, For, onCleanup, Show } from "solid-js";
import { api, artUrl, errorText, type CoverCandidate } from "../api";
import { library } from "../state/library";
import { osk } from "../state/osk";
import { bumpCaseRev, caseRev } from "../state/fx";
import { applyCall, CASE_SLOTS, FILE_SLOTS, REMOVABLE, type Pick, type Slot } from "../state/artworkStudio";
import { NavScope } from "./NavScope";
import { FileBrowser } from "./FileBrowser";
import { Hint } from "./Hints";
import "../styles/artwork.css";

const ORDER: Slot[] = ["cover", "hero", "logo", "icon", "back", "spine", "disc", "manual"];
/** Wide pictures on the top row, the case and icon below. */
const TOP: Slot[] = ["hero", "logo"];
const BOTTOM: Slot[] = ["cover", "back", "spine", "disc", "icon", "manual"];
const NAMES: Record<Slot, string> = {
  cover: "Front cover",
  hero: "Background",
  logo: "Logo",
  icon: "Icon",
  back: "Back cover",
  spine: "Spine",
  disc: "Disc",
  manual: "Manual",
};

const mb = (n: number | null) => (n ? ` · ${Math.max(1, Math.round(n / 1048576))} MB` : "");

/**
 * The artwork studio: every picture of one game on one screen. Pick a card to
 * choose its options; a choice shows on its card as NEW right away and is saved
 * only by Apply, so several pictures can change at once.
 */
export function ArtworkStudio(props: { gameId: number; title: string; cart: boolean; onClose: () => void; onChanged: () => void }) {
  const row = () => library.games().find((g) => g.id === props.gameId);
  const nameOf = (s: Slot) => (s === "disc" && props.cart ? "Cartridge" : NAMES[s]);

  const [active, setActive] = createSignal<Slot>("cover");
  const [staged, setStaged] = createSignal<Partial<Record<Slot, Pick>>>({});
  // "Show current" in place of the staged choice, for the active slot only.
  const [peek, setPeek] = createSignal(false);
  const [queries, setQueries] = createSignal<Partial<Record<Slot, string>>>({});
  const [tick, setTick] = createSignal(0);
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [toast, setToast] = createSignal<string | null>(null);
  const [browsing, setBrowsing] = createSignal(false);
  const [loadingGame, setLoadingGame] = createSignal<string | null>(null);
  // Previews that failed to load (GameTDB guesses per region).
  const [broken, setBroken] = createSignal<ReadonlySet<string>>(new Set());
  let alive = true;
  onCleanup(() => (alive = false));

  const query = () => queries()[active()] ?? null;
  const [candidates] = createResource(
    () => ({ id: props.gameId, slot: active(), q: query(), t: tick() }),
    ({ id, slot, q }) =>
      (slot === "manual"
        ? api.manualCandidates(id, q).then((list) => list.map((e): CoverCandidate => ({ source: "manual", name: e.name, url: e.url, preview: null, label: e.label + mb(e.size) })))
        : api.coverCandidates(id, q, slot)
      ).catch((e) => Promise.reject(new Error(errorText(e)))),
  );
  // The manual has no picture: its card says where it comes from.
  const [manualNow, { refetch: refetchManual }] = createResource(() => props.gameId, (id) => api.manualStatus(id).catch(() => undefined));
  const options = () =>
    candidates.state === "ready" ? candidates().filter((c) => !broken().has(c.url)) : [];

  /** The picture a slot shows now, or null when it has none. */
  const currentUrl = (s: Slot): string | null => {
    const r = row();
    if (!r) return null;
    switch (s) {
      case "cover": return r.art > 0 ? artUrl(r.id, r.art, "f") : null;
      case "hero": return r.hero ? artUrl(r.id, r.hero, "h") : null;
      case "logo": return r.logo ? artUrl(r.id, r.logo, "l") : null;
      case "icon": return r.icon ? artUrl(r.id, r.icon, "i") : null;
      case "back": return artUrl(r.id, caseRev(), "b");
      case "spine": return artUrl(r.id, caseRev(), "s");
      case "disc": return artUrl(r.id, caseRev(), "d");
      case "manual": return null;
    }
  };

  const manualText = () => {
    const s = manualNow();
    return s?.source ?? (s?.state === "off" ? "None (downloads off)" : s?.state === "none" ? "None found" : "Looked up when opened");
  };

  /** The picture a slot shows: its staged choice, or the current one. */
  const shown = (s: Slot): { url: string | null; text: string | null; staged: boolean } => {
    const p = staged()[s];
    if (!p || (peek() && active() === s)) return { url: currentUrl(s), text: s === "manual" ? manualText() : null, staged: false };
    switch (p.op) {
      case "option": return { url: p.c.preview, text: p.c.label, staged: true };
      case "file": return { url: null, text: s === "manual" ? "Your file" : "Your image", staged: true };
      case "remove": return { url: null, text: "Removed", staged: true };
      case "useCover": return { url: currentUrl("cover"), text: "The cover", staged: true };
    }
  };

  const pending = () => Object.keys(staged()).length;
  const isStaged = (c: CoverCandidate) => {
    const p = staged()[active()];
    return p?.op === "option" && p.c.url === c.url;
  };

  function choose(p: Pick) {
    setStaged({ ...staged(), [active()]: p });
    setPeek(false);
    setToast(null);
  }

  function pick(c: CoverCandidate) {
    if (c.source === "lbgame") void loadGame(c);
    else choose({ op: "option", c });
  }

  // A LaunchBox entry found by name: its pictures download in the background (the first time
  // that's LaunchBox's ~100 MB file), then replace the entry in the list.
  async function loadGame(c: CoverCandidate) {
    setError(null);
    setLoadingGame(c.name);
    try {
      await api.lbBrowse(options().filter((x) => x.source === "lbgame").map((x) => Number(x.url)));
      for (let i = 0; i < 450 && alive; i++) {
        await new Promise((r) => setTimeout(r, 2000));
        const now = await api.coverCandidates(props.gameId, query(), active() as Exclude<Slot, "manual">);
        if (!now.some((x) => x.source === "lbgame")) {
          if (alive) setTick((t) => t + 1);
          return;
        }
      }
      setError("LaunchBox's pictures haven't arrived yet. Check the progress in the footer and try again.");
    } catch (e) {
      setError(errorText(e));
    } finally {
      setLoadingGame(null);
    }
  }

  function search() {
    osk.open({
      title: `Search ${NAMES[active()].toLowerCase()} by name (e.g. the original game for a hack of ${props.title})`,
      value: query() ?? "",
      onDone: (v) => setQueries({ ...queries(), [active()]: v.trim() || undefined }),
    });
  }

  function cancelAll() {
    setStaged({});
    setPeek(false);
  }

  function undo() {
    const next = { ...staged() };
    delete next[active()];
    setStaged(next);
    setPeek(false);
  }

  /** Saves every staged choice, one call each; stops at the first failure and keeps the rest staged. */
  async function applyAll() {
    const entries = Object.entries(staged()) as [Slot, Pick][];
    if (!entries.length || busy()) return;
    setBusy(true);
    setError(null);
    let saved = 0;
    try {
      for (const [slot, p] of entries) {
        const call = applyCall(slot, p);
        if (call.op === "manualPick") await api.manualChoose(props.gameId, call.url, call.label);
        else if (call.op === "manualFile") await api.manualChooseFile(props.gameId, call.path);
        else if (call.op === "manualRemove") await api.manualRemove(props.gameId);
        else if (call.op === "set") await api.setCover(props.gameId, call.source, call.name, call.url, slot as Exclude<Slot, "manual">);
        else if (call.op === "removeCover") await api.removeCover(props.gameId);
        else if (call.op === "removeExtra") await api.removeExtra(props.gameId, call.kind);
        else await api.useCoverIcon(props.gameId);
        if (CASE_SLOTS.includes(slot)) bumpCaseRev();
        if (slot === "manual") void refetchManual();
        const next = { ...staged() };
        delete next[slot];
        setStaged(next);
        saved++;
      }
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
      if (saved) {
        setToast(saved === 1 ? "1 picture saved" : `${saved} pictures saved`);
        props.onChanged();
      }
    }
  }

  function step(delta: 1 | -1) {
    const i = ORDER.indexOf(active());
    setActive(ORDER[(i + delta + ORDER.length) % ORDER.length]);
    setPeek(false);
  }

  const canRemove = () =>
    REMOVABLE.includes(active()) && (active() === "manual" ? ["ready", "available"].includes(manualNow()?.state ?? "") : !!currentUrl(active()));
  const canFile = () => FILE_SLOTS.includes(active());

  const card = (s: Slot) => {
    const view = () => shown(s);
    return (
      <button
        class="art-card"
        classList={{ on: active() === s, top: TOP.includes(s), staged: !!staged()[s] }}
        aria-label={nameOf(s)}
        aria-pressed={active() === s}
        onClick={() => {
          setActive(s);
          setPeek(false);
        }}
      >
        <div class="art-well">
          <Show when={view().url && !broken().has(view().url!)} fallback={<span class="art-empty">{view().text ?? "None"}</span>}>
            <img
              src={view().url!}
              alt=""
              loading="lazy"
              decoding="async"
              onError={() => setBroken(new Set(broken()).add(view().url!))}
            />
          </Show>
        </div>
        <div class="art-foot">
          <span class="art-name">{nameOf(s)}</span>
          <Show when={view().staged}>
            <span class="art-tag">{peek() && active() === s ? "Current" : "New"}</span>
          </Show>
        </div>
      </button>
    );
  };

  return (
    <div class="overlay" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <NavScope
        class="panel art-studio"
        modal
        onBack={() => (pending() ? cancelAll() : props.onClose())}
        onTab={step}
        onSecondary={search}
        onMenu={() => void applyAll()}
      >
        <header class="art-head">
          <div>
            <h2>Artwork</h2>
            <p class="dim small">{props.title}</p>
          </div>
          <span class="art-head-status dim small">{pending() ? `${pending()} changed` : "No changes"}</span>
          <button class="btn" disabled={!pending() || busy()} onClick={cancelAll}>Cancel</button>
          <button class="btn primary" disabled={!pending() || busy()} onClick={() => void applyAll()}>
            {pending() ? `Apply ${pending()}` : "Apply"}
          </button>
        </header>
        <Show when={error()}>
          <p class="warn">{error()}</p>
        </Show>

        <div class="art-body">
          <div class="art-board">
            <div class="art-row top">{TOP.map(card)}</div>
            <div class="art-row bottom">{BOTTOM.map(card)}</div>
            <p class="dim small">Pick a picture, then a choice from the list. Choices show here straight away, marked New; Apply saves them all.</p>
          </div>

          <aside class="art-panel">
            <div class="art-panel-head">
              <h3>{nameOf(active())}</h3>
              <span class="dim small">
                {candidates.loading ? "Looking…" : candidates.state === "ready" ? `${options().length} options` : ""}
              </span>
            </div>
            <div class="row tight">
              <button class="btn small" onClick={search}>Search…</button>
              <Show when={canFile()}>
                <button class="btn small" onClick={() => setBrowsing(true)}>{active() === "manual" ? "PDF or CBZ file…" : "Image file…"}</button>
              </Show>
            </div>
            <Show when={candidates.error}>
              <p class="warn small">Couldn't load options: {String(candidates.error?.message ?? candidates.error)}</p>
            </Show>
            <Show when={loadingGame()}>
              {(n) => <p class="dim small">Loading {n()}'s pictures from LaunchBox… the first time downloads its file (about 100 MB; progress is in the footer).</p>}
            </Show>
            <div class="art-options">
              <For each={options()}>
                {(c) => (
                  <button class="art-opt" classList={{ staged: isStaged(c) }} disabled={!!loadingGame() || busy()} onClick={() => pick(c)} title={c.label}>
                    <div class="art-opt-thumb">
                      <Show when={c.preview} fallback={<span class="art-empty">{c.label}</span>}>
                        <img src={c.preview!} alt="" loading="lazy" decoding="async" onError={() => c.source === "gametdb" && setBroken(new Set(broken()).add(c.url))} />
                      </Show>
                    </div>
                    <span class="art-opt-label">{c.label}</span>
                    <Show when={isStaged(c)}>
                      <span class="art-tag">New</span>
                    </Show>
                  </button>
                )}
              </For>
              <Show when={candidates.state === "ready" && options().length === 0}>
                <p class="dim small">No {NAMES[active()].toLowerCase()} found. Try searching with a different name.</p>
              </Show>
            </div>
            <Show when={active() === "icon"}>
              <button class="btn small" disabled={busy()} onClick={() => choose({ op: "useCover" })}>Use the cover</button>
            </Show>
            <Show when={canRemove()}>
              <button class="btn small danger" disabled={busy()} onClick={() => choose({ op: "remove" })}>
                {active() === "cover" ? "Remove cover" : active() === "icon" ? "Remove icon" : active() === "hero" ? "No background" : active() === "manual" ? "Forget this manual" : "No logo"}
              </button>
            </Show>
            <Show when={staged()[active()]}>
              <div class="art-compare">
                <button class="btn small" classList={{ primary: !peek() }} onClick={() => setPeek(false)}>Show new</button>
                <button class="btn small" classList={{ primary: peek() }} onClick={() => setPeek(true)}>Show current</button>
                <button class="btn small" onClick={undo}>Undo this change</button>
              </div>
            </Show>
          </aside>
        </div>

        <div class="hints-row">
          <Hint actions={["prevPlatform", "nextPlatform"]} label="Picture" />
          <Hint action="select" label="Choose" />
          <Hint action="secondary" label="Search" />
          <Hint action="menu" label="Apply" />
          <Hint action="back" label="Cancel / Close" />
        </div>
        <Show when={toast()}>
          <p class="art-toast">{toast()}</p>
        </Show>
      </NavScope>
      <Show when={browsing()}>
        <FileBrowser
          title={active() === "manual" ? "Choose the manual (a PDF or CBZ file)" : `Choose an image for the ${NAMES[active()].toLowerCase()}`}
          mode="file"
          extensions={active() === "manual" ? ["pdf", "cbz"] : ["png", "jpg", "jpeg", "webp"]}
          onPick={(path) => {
            setBrowsing(false);
            choose({ op: "file", path });
          }}
          onClose={() => setBrowsing(false)}
        />
      </Show>
    </div>
  );
}
