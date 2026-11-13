import { createResource, createSignal, For, Show } from "solid-js";
import { api, errorText, type DirEntry, type DirListing } from "../api";
import { NavScope } from "./NavScope";
import { Hint } from "./Hints";

interface Props {
  title: string;
  /** Where to start; a missing path opens its nearest existing folder. */
  start?: string | null;
  /** "folder": pick a directory. "file": pick a file with one of `extensions`. */
  mode: "folder" | "file";
  extensions?: string[];
  onPick: (path: string) => void;
  onClose: () => void;
}

/**
 * In-app file/folder picker, usable with a controller (the native Windows
 * dialog isn't). A enters a folder or picks a file, B goes up, □ / X picks
 * the current folder.
 */
export function FileBrowser(props: Props) {
  const [path, setPath] = createSignal<string | null>(props.start?.trim() || null);
  const [lastGood, setLastGood] = createSignal<DirListing | undefined>();
  const [error, setError] = createSignal<string | null>(null);
  // Wrapped in an object so a null path (the drive list) still triggers a fetch.
  // Errors are returned, not thrown: an errored resource throws when read.
  createResource(
    () => ({ p: path() }),
    async ({ p }) => {
      try {
        const l = await api.listDir(p, props.mode === "file" ? props.extensions ?? [] : []);
        setLastGood(l);
        setError(null);
        // Land on the first entry of each listing (it renders after this resolves).
        queueMicrotask(() => scopeEl?.querySelector<HTMLElement>(".fb-entry")?.focus({ preventScroll: true }));
      } catch (e) {
        setError(errorText(e));
      }
      return true;
    },
  );
  const current = lastGood;
  let scopeEl!: HTMLDivElement;

  const goTo = (p: string | null) => setPath(p);
  const up = () => {
    const l = current();
    if (!l || l.path === null) props.onClose();
    else goTo(l.parent);
  };
  const open = (e: DirEntry) => (e.isDir ? goTo(e.path) : props.onPick(e.path));
  const pickHere = () => {
    const p = current()?.path;
    if (props.mode === "folder" && p) props.onPick(p);
  };

  return (
    <div class="overlay" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <NavScope
        class="panel fb"
        modal
        ref={(el) => (scopeEl = el)}
        onBack={up}
        onSecondary={props.mode === "folder" ? pickHere : undefined}
      >
        <h3>{props.title}</h3>
        <div class="fb-path mono">{current()?.path ?? "This PC"}</div>
        <Show when={error()}>
          <p class="warn">{error()}</p>
        </Show>
        <div class="fb-list">
          <Show when={current()?.path}>
            <button class="fb-entry dir" onClick={up}>
              ↑ Up
            </button>
          </Show>
          <For each={current()?.entries ?? []}>
            {(e) => (
              <button class="fb-entry" classList={{ dir: e.isDir }} onClick={() => open(e)}>
                <span class="fb-icon" aria-hidden="true">
                  {e.isDir ? "📁" : "▶"}
                </span>
                {e.name}
              </button>
            )}
          </For>
          <Show when={current() && current()!.entries.length === 0}>
            <p class="dim">{props.mode === "file" ? "No matching files here." : "No subfolders."}</p>
          </Show>
          <Show when={current()?.truncated}>
            <p class="dim small">Showing the first 3000 entries.</p>
          </Show>
        </div>
        <div class="panel-actions">
          <div class="hints-row">
            <Hint action="select" label={props.mode === "file" ? "Open / pick" : "Open"} />
            <Hint action="back" label="Up" />
            <Show when={props.mode === "folder"}>
              <Hint action="secondary" label="Use this folder" />
            </Show>
          </div>
          <Show when={props.mode === "folder"}>
            <button class="btn primary" disabled={!current()?.path} onClick={pickHere}>
              Use this folder
            </button>
          </Show>
          <button class="btn" onClick={props.onClose}>
            Cancel
          </button>
        </div>
      </NavScope>
    </div>
  );
}
