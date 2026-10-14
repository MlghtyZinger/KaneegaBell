import { createResource, createSignal, For, Show } from "solid-js";
import { api, errorText } from "../api";
import { osk } from "../state/osk";
import { NavScope } from "./NavScope";
import { Hint } from "./Hints";

type Source = "launchbox" | "igdb";

/**
 * "Fix match": pick the entry this game really is, in the LaunchBox Games
 * Database (keyless, searched offline in the stored name list) or on IGDB
 * (with a key). The pick is kept for good (automatic lookups never replace it).
 */
export function MatchPicker(props: { gameId: number; title: string; igdb: boolean; onClose: () => void; onChanged: () => void }) {
  const [source, setSource] = createSignal<Source>("launchbox");
  const [query, setQuery] = createSignal<string | null>(null);
  const [lbResults] = createResource(
    () => (source() === "launchbox" ? { id: props.gameId, q: query() } : false),
    ({ id, q }) => api.lbSearch(id, q).catch((e) => Promise.reject(new Error(errorText(e)))),
  );
  const [igdbResults] = createResource(
    () => (source() === "igdb" ? { id: props.gameId, q: query() } : false),
    ({ id, q }) => api.igdbSearch(id, q).catch((e) => Promise.reject(new Error(errorText(e)))),
  );
  const results = () => (source() === "launchbox" ? lbResults : igdbResults);
  const [busy, setBusy] = createSignal<number | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  async function choose(id: number) {
    setBusy(id);
    setError(null);
    try {
      if (source() === "launchbox") await api.setLbMatch(props.gameId, id);
      else await api.setIgdbMatch(props.gameId, id);
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
      title: source() === "launchbox" ? "Search LaunchBox by name" : "Search IGDB by name",
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
          <Show when={props.igdb}>
            <button class="tab" classList={{ active: source() === "launchbox" }} onClick={() => setSource("launchbox")}>
              LaunchBox
            </button>
            <button class="tab" classList={{ active: source() === "igdb" }} onClick={() => setSource("igdb")}>
              IGDB
            </button>
          </Show>
          <button class="btn" onClick={search}>
            Search by name…
          </button>
        </div>
        <Show when={error()}>
          <p class="warn">{error()}</p>
        </Show>
        <Show when={results().loading}>
          <p class="dim">Searching…</p>
        </Show>
        <Show when={results().error}>
          <p class="warn">{String(results().error?.message ?? results().error)}</p>
        </Show>
        <div class="match-list">
          <Show when={source() === "launchbox"}>
            <Show when={lbResults.state === "ready" && lbResults()!.length === 0}>
              <p class="dim">
                No results. Try another name (the original title of a hack, or another region's name). If LaunchBox's
                data hasn't been downloaded yet, use Update now in Settings → Game info.
              </p>
            </Show>
            <For each={lbResults.state === "ready" ? lbResults() : []}>
              {(c, i) => (
                <button
                  class="match-option"
                  data-autofocus={i() === 0 ? "" : undefined}
                  disabled={busy() !== null}
                  onClick={() => void choose(c.dbId)}
                >
                  <span class="match-text">
                    <span class="match-name">{busy() === c.dbId ? "Saving…" : c.name}</span>
                    <Show when={c.alsoKnownAs}>
                      <span class="match-summary dim">Also known as {c.alsoKnownAs}</span>
                    </Show>
                  </span>
                </button>
              )}
            </For>
          </Show>
          <Show when={source() === "igdb"}>
            <Show when={igdbResults.state === "ready" && igdbResults()!.length === 0}>
              <p class="dim">No results. Try another name (the original title of a hack, or the Japanese name).</p>
            </Show>
            <For each={igdbResults.state === "ready" ? igdbResults() : []}>
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
          </Show>
        </div>
        <div class="panel-actions">
          <div class="hints-row">
            <Hint action="select" label="This is the game" />
            <Hint action="secondary" label="Search" />
            <Hint action="back" label="Close" />
          </div>
          <span class="dim small">{source() === "launchbox" ? "From the LaunchBox Games Database" : "Game info from IGDB.com"}</span>
          <button class="btn" onClick={props.onClose}>
            Close
          </button>
        </div>
      </NavScope>
    </div>
  );
}
