import { createResource, createSignal, For, Show } from "solid-js";
import { api, errorText, type CoverCandidate } from "../api";
import { osk } from "../state/osk";
import { NavScope } from "./NavScope";
import { FileBrowser } from "./FileBrowser";
import { Hint } from "./Hints";

/**
 * "Change cover": pick from libretro alternatives (other regions, related
 * titles), search the platform's whole cover set by name, use the game's own
 * icon, pick an image file, or remove the cover. Choices stick: automatic
 * fetching never replaces them.
 */
export function CoverPicker(props: { gameId: number; title: string; onClose: () => void; onChanged: () => void }) {
  const [query, setQuery] = createSignal<string | null>(null);
  const [candidates] = createResource(
    () => ({ id: props.gameId, q: query() }),
    ({ id, q }) => api.coverCandidates(id, q).catch((e) => Promise.reject(new Error(errorText(e)))),
  );
  const [busy, setBusy] = createSignal<string | null>(null);
  const [error, setError] = createSignal<string | null>(null);
  const [browsing, setBrowsing] = createSignal(false);

  async function choose(source: CoverCandidate["source"] | "file", name: string, url: string) {
    setBusy(name);
    setError(null);
    try {
      await api.setCover(props.gameId, source, name, url);
      props.onChanged();
      props.onClose();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(null);
    }
  }

  async function remove() {
    await api.removeCover(props.gameId);
    props.onChanged();
    props.onClose();
  }

  function search() {
    osk.open({
      title: `Search covers by name (e.g. the original game for a hack of ${props.title})`,
      value: query() ?? "",
      onDone: (v) => setQuery(v.trim() || null),
    });
  }

  // GameTDB options are guesses per region: drop the ones whose image doesn't exist.
  const [broken, setBroken] = createSignal<ReadonlySet<string>>(new Set());
  const list = (): CoverCandidate[] =>
    candidates.state === "ready" ? candidates().filter((c) => !broken().has(c.url)) : [];

  return (
    <div class="overlay" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <NavScope class="panel cover-picker" modal onBack={props.onClose} onSecondary={search}>
        <h2>Change cover</h2>
        <p class="dim">
          {props.title}
          {query() ? ` · results for “${query()}”` : ""}
        </p>
        <div class="row">
          <button class="btn" onClick={search}>
            Search covers…
          </button>
          <button class="btn" onClick={() => setBrowsing(true)}>
            Use an image file…
          </button>
          <button class="btn danger" onClick={() => void remove()}>
            Remove cover
          </button>
        </div>
        <Show when={error()}>
          <p class="warn">{error()}</p>
        </Show>
        <Show when={candidates.loading}>
          <p class="dim">Looking for covers…</p>
        </Show>
        <Show when={candidates.error}>
          <p class="warn">Couldn't load covers: {String(candidates.error?.message ?? candidates.error)}</p>
        </Show>
        <Show when={candidates.state === "ready" && list().length === 0}>
          <p class="dim">No covers found. Try searching with a different name.</p>
        </Show>
        <div class="cover-grid">
          <For each={list()}>
            {(c, i) => (
              <button
                class="cover-option"
                data-autofocus={i() === 0 ? "" : undefined}
                disabled={busy() !== null}
                onClick={() => void choose(c.source, c.name, c.url)}
                title={c.label}
              >
                <Show when={c.preview} fallback={<div class="cover-option-icon">🎮</div>}>
                  <img
                    src={c.preview!}
                    alt=""
                    loading="lazy"
                    decoding="async"
                    onError={() => c.source === "gametdb" && setBroken((s) => new Set(s).add(c.url))}
                  />
                </Show>
                <span class="cover-option-label">{busy() === c.name ? "Applying…" : c.label}</span>
              </button>
            )}
          </For>
        </div>
        <div class="panel-actions">
          <div class="hints-row">
            <Hint action="select" label="Use this cover" />
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
          title="Choose a cover image"
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
