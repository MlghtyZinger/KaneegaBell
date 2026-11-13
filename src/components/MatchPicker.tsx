import { createResource, createSignal, For, Show } from "solid-js";
import { api, errorText } from "../api";
import { osk } from "../state/osk";
import { NavScope } from "./NavScope";
import { Hint } from "./Hints";

/**
 * "Fix match": search IGDB and pick the entry this game really is. The pick
 * is kept for good (automatic lookups never replace it).
 */
export function MatchPicker(props: { gameId: number; title: string; onClose: () => void; onChanged: () => void }) {
  const [query, setQuery] = createSignal<string | null>(null);
  const [results] = createResource(
    () => ({ id: props.gameId, q: query() }),
    ({ id, q }) => api.igdbSearch(id, q).catch((e) => Promise.reject(new Error(errorText(e)))),
  );
  const [busy, setBusy] = createSignal<number | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  async function choose(igdbId: number) {
    setBusy(igdbId);
    setError(null);
    try {
      await api.setIgdbMatch(props.gameId, igdbId);
      props.onChanged();
      props.onClose();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(null);
    }
  }

  function search() {
    osk.open({
      title: "Search IGDB by name",
      value: "",
      onDone: (v) => setQuery(v.trim() || null),
    });
  }

  return (
    <div class="overlay" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <NavScope class="panel match-picker" modal onBack={props.onClose} onSecondary={search}>
        <h2>Fix match</h2>
        <p class="dim">
          {props.title}
          {query() ? ` · results for “${query()}”` : ""}
        </p>
        <div class="row">
          <button class="btn" onClick={search}>
            Search by name…
          </button>
        </div>
        <Show when={error()}>
          <p class="warn">{error()}</p>
        </Show>
        <Show when={results.loading}>
          <p class="dim">Searching IGDB…</p>
        </Show>
        <Show when={results.error}>
          <p class="warn">{String(results.error?.message ?? results.error)}</p>
        </Show>
        <Show when={results.state === "ready" && results().length === 0}>
          <p class="dim">No results. Try another name (the original title of a hack, or the Japanese name).</p>
        </Show>
        <div class="match-list">
          <For each={results.state === "ready" ? results() : []}>
            {(c, i) => (
              <button
                class="match-option"
                data-autofocus={i() === 0 ? "" : undefined}
                disabled={busy() !== null}
                onClick={() => void choose(c.igdbId)}
              >
                <Show when={c.cover} fallback={<div class="match-cover empty" />}>
                  <img class="match-cover" src={c.cover!} alt="" loading="lazy" decoding="async" />
                </Show>
                <span class="match-text">
                  <span class="match-name">
                    {busy() === c.igdbId ? "Saving…" : c.name}
                    {c.year ? <span class="dim"> ({c.year})</span> : null}
                  </span>
                  <Show when={c.onPlatform}>
                    <span class="badge">This platform</span>
                  </Show>
                  <Show when={c.summary}>
                    <span class="match-summary dim">{c.summary}</span>
                  </Show>
                </span>
              </button>
            )}
          </For>
        </div>
        <div class="panel-actions">
          <div class="hints-row">
            <Hint action="select" label="This is the game" />
            <Hint action="secondary" label="Search" />
            <Hint action="back" label="Close" />
          </div>
          <span class="dim small">Game info from IGDB.com</span>
          <button class="btn" onClick={props.onClose}>
            Close
          </button>
        </div>
      </NavScope>
    </div>
  );
}
