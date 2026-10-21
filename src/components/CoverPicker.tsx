import { createResource, createSignal, For, onCleanup, Show } from "solid-js";
import { api, errorText, type CoverCandidate } from "../api";
import { osk } from "../state/osk";
import { bumpCaseRev } from "../state/fx";
import { NavScope } from "./NavScope";
import { FileBrowser } from "./FileBrowser";
import { Hint } from "./Hints";

export type PictureKind = "cover" | "hero" | "logo" | "icon" | "back" | "disc" | "spine";
/** The pictures on a game's 3D case besides the cover: LaunchBox's, or an image file of your own. */
const CASE_ART: PictureKind[] = ["back", "disc", "spine"];

const WORDS: Record<PictureKind, { title: string; noun: string; use: string }> = {
  cover: { title: "Change cover", noun: "covers", use: "Use this cover" },
  hero: { title: "Change background", noun: "backgrounds", use: "Use this background" },
  logo: { title: "Change logo", noun: "logos", use: "Use this logo" },
  icon: { title: "Change icon", noun: "icons", use: "Use this icon" },
  back: { title: "Change back cover", noun: "back covers", use: "Use this back cover" },
  disc: { title: "Change disc", noun: "discs", use: "Use this disc" },
  spine: { title: "Change spine", noun: "spines", use: "Use this spine" },
};

/**
 * A background's shape: "16:9", or "3:1 wide" past the 2:1 cut-off SideHero
 * uses to show it whole with bands instead of filling the sidebar's box.
 */
export function shapeOf(img: HTMLImageElement): string {
  const r = img.naturalWidth / img.naturalHeight;
  const name = Math.abs(r - 16 / 9) < 0.12 ? "16:9" : r >= 2.8 && r <= 3.3 ? "3:1" : `${r.toFixed(1)}:1`;
  return r > 2 ? `${name} wide` : name;
}

/**
 * "Change cover": pick from libretro alternatives (other regions, related
 * titles), SteamGridDB (with a key), search by name, use the game's own icon,
 * pick an image file, or remove the cover. With `kind` "hero" / "logo" it
 * picks TV Mode's background or logo from SteamGridDB instead. Choices stick:
 * automatic fetching never replaces them.
 */
export function CoverPicker(props: {
  gameId: number;
  title: string;
  kind?: PictureKind;
  /** A cartridge system: the "disc" is the game's cartridge. */
  cart?: boolean;
  onClose: () => void;
  onChanged: () => void;
}) {
  const kind = () => props.kind ?? "cover";
  const words = () => (props.cart && kind() === "disc" ? { title: "Change cartridge", noun: "cartridges", use: "Use this cartridge" } : WORDS[kind()]);
  const [query, setQuery] = createSignal<string | null>(null);
  // Bumped when a LaunchBox entry's pictures have arrived, to list them.
  const [tick, setTick] = createSignal(0);
  const [candidates] = createResource(
    () => ({ id: props.gameId, q: query(), k: kind(), t: tick() }),
    ({ id, q, k }) => api.coverCandidates(id, q, k).catch((e) => Promise.reject(new Error(errorText(e)))),
  );
  const [busy, setBusy] = createSignal<string | null>(null);
  const [error, setError] = createSignal<string | null>(null);
  const [browsing, setBrowsing] = createSignal(false);

  async function choose(source: CoverCandidate["source"] | "file", name: string, url: string) {
    setBusy(name);
    setError(null);
    try {
      await api.setCover(props.gameId, source, name, url, kind());
      if (CASE_ART.includes(kind())) bumpCaseRev();
      props.onChanged();
      props.onClose();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(null);
    }
  }

  // A LaunchBox entry found by name: its pictures are fetched in the background (the first time that
  // downloads LaunchBox's file, ~100 MB), then they replace the entry in the list.
  const [loadingGame, setLoadingGame] = createSignal<string | null>(null);
  let alive = true;
  onCleanup(() => (alive = false));
  async function loadGame(c: CoverCandidate) {
    setError(null);
    setLoadingGame(c.name);
    try {
      // Every entry in the list is asked for at once: one download covers them all.
      await api.lbBrowse(list().filter((x) => x.source === "lbgame").map((x) => Number(x.url)));
      for (let i = 0; i < 450 && alive; i++) {
        await new Promise((r) => setTimeout(r, 2000));
        const now = await api.coverCandidates(props.gameId, query(), kind());
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

  async function remove() {
    if (kind() === "cover") await api.removeCover(props.gameId);
    else await api.removeExtra(props.gameId, kind() as "hero" | "logo" | "icon");
    props.onChanged();
    props.onClose();
  }

  function search() {
    osk.open({
      title: CASE_ART.includes(kind())
        ? `Search ${words().noun} by game name (LaunchBox)`
        : `Search ${words().noun} by name (e.g. the original game for a hack of ${props.title})`,
      value: query() ?? "",
      onDone: (v) => setQuery(v.trim() || null),
    });
  }

  // GameTDB options are guesses per region: drop the ones whose image doesn't exist.
  const [broken, setBroken] = createSignal<ReadonlySet<string>>(new Set());
  // Each background's shape, read from its preview once it loads.
  const [shapes, setShapes] = createSignal<Record<string, string>>({});
  const list = (): CoverCandidate[] =>
    candidates.state === "ready" ? candidates().filter((c) => !broken().has(c.url)) : [];

  return (
    <div class="overlay" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <NavScope class="panel cover-picker" modal onBack={props.onClose} onSecondary={search}>
        <h2>{words().title}</h2>
        <p class="dim">
          {props.title}
          {query() ? ` · results for “${query()}”` : ""}
        </p>
        <div class="row">
          <button class="btn" onClick={search}>
            Search {words().noun}…
          </button>
          <Show when={kind() === "cover" || kind() === "icon" || CASE_ART.includes(kind())}>
            <button class="btn" onClick={() => setBrowsing(true)}>
              Use an image file…
            </button>
          </Show>
          <Show when={kind() === "icon"}>
            <button class="btn" onClick={async () => { await api.useCoverIcon(props.gameId); props.onChanged(); props.onClose(); }}>Use the cover</button>
          </Show>
          <Show when={!CASE_ART.includes(kind())}>
            <button class="btn danger" onClick={() => void remove()}>
              {kind() === "cover" ? "Remove cover" : kind() === "hero" ? "No background" : kind() === "icon" ? "Remove icon" : "No logo"}
            </button>
          </Show>
        </div>
        <Show when={error()}>
          <p class="warn">{error()}</p>
        </Show>
        <Show when={loadingGame()}>
          {(n) => <p class="dim">Loading {n()}'s pictures from LaunchBox… the first time downloads its file (about 100 MB; progress is in the footer).</p>}
        </Show>
        <Show when={candidates.loading}>
          <p class="dim">Looking for {words().noun}…</p>
        </Show>
        <Show when={candidates.error}>
          <p class="warn">Couldn't load {words().noun}: {String(candidates.error?.message ?? candidates.error)}</p>
        </Show>
        <Show when={candidates.state === "ready" && list().length === 0}>
          <p class="dim">No {words().noun} found. Try searching with a different name{query() && CASE_ART.includes(kind()) ? " (LaunchBox has none for those entries)" : ""}.</p>
        </Show>
        <Show when={kind() === "hero"}>
          <p class="dim small">16:9 backgrounds fill the sidebar; wide ones show whole, with blurred bands above and below.</p>
        </Show>
        <div class="cover-grid" classList={{ wide: kind() === "hero", logos: kind() === "logo" || kind() === "spine", icons: kind() === "icon" || kind() === "disc" }}>
          <For each={list()}>
            {(c, i) => (
              <button
                class="cover-option"
                data-autofocus={i() === 0 ? "" : undefined}
                disabled={busy() !== null || loadingGame() !== null}
                onClick={() => (c.source === "lbgame" ? void loadGame(c) : void choose(c.source, c.name, c.url))}
                title={c.label}
              >
                <Show when={c.preview} fallback={<div class="cover-option-icon">🎮</div>}>
                  <img
                    src={c.preview!}
                    alt=""
                    loading="lazy"
                    decoding="async"
                    onError={() => c.source === "gametdb" && setBroken((s) => new Set(s).add(c.url))}
                    onLoad={(e) => kind() === "hero" && setShapes((m) => ({ ...m, [c.url]: shapeOf(e.currentTarget) }))}
                  />
                </Show>
                <span class="cover-option-label">
                  {busy() === c.name ? "Applying…" : c.label}
                  <Show when={shapes()[c.url]}>{(sh) => <b class="cover-option-shape"> · {sh()}</b>}</Show>
                </span>
              </button>
            )}
          </For>
        </div>
        <div class="panel-actions">
          <div class="hints-row">
            <Hint action="select" label={words().use} />
            <Hint action="secondary" label="Search" />
            <Hint action="back" label="Close" />
          </div>
          <button class="btn" onClick={props.onClose}>
            Close
          </button>
        </div>
      </NavScope>
      <Show when={browsing()}>
        <FileBrowser
          title={CASE_ART.includes(kind()) ? `Choose an image for the ${kind() === "back" ? "back cover" : kind()}` : "Choose a cover image"}
          mode="file"
          extensions={["png", "jpg", "jpeg", "webp"]}
          onPick={(path) => {
            setBrowsing(false);
            void choose("file", path, path);
          }}
          onClose={() => setBrowsing(false)}
        />
      </Show>
    </div>
  );
}
