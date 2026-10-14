import { createSignal, For, Show } from "solid-js";
import { api, artUrl, errorText, type Collection } from "../api";
import { collections } from "../state/collections";
import { library } from "../state/library";
import { osk } from "../state/osk";
import { NavScope } from "./NavScope";
import { FileBrowser } from "./FileBrowser";
import { Hint } from "./Hints";

const KIND_LABEL = { mine: "Collection", series: "Series", franchise: "Franchise" } as const;

/** The collection's picture: the chosen logo, else (series) its first logo, else the name. */
function Emblem(props: { c: Collection; class?: string }) {
  const chosen = () => collections.chosenPicture(props.c);
  const auto = () => collections.autoLogo(props.c);
  return (
    <Show
      when={chosen()?.kind === "logo" ? chosen()!.url : !chosen() ? auto() : null}
      fallback={<span class={`col-name ${props.class ?? ""}`}>{props.c.name}</span>}
    >
      {(url) => <img class={`col-logo ${props.class ?? ""}`} src={url()} alt={props.c.name} decoding="async" draggable={false} />}
    </Show>
  );
}

/** A tile in the Collections tab: a cover mosaic (or the chosen cover) with its logo or name. */
export function CollectionTile(props: { c: Collection; focused: () => boolean }) {
  const chosen = () => collections.chosenPicture(props.c);
  const covers = () => collections.mosaic(props.c);
  return (
    <div class="col-tile" classList={{ focused: props.focused(), hidden: props.c.hidden }}>
      <Show
        when={chosen()?.kind === "cover"}
        fallback={
          <div class="col-mosaic" classList={{ one: covers().length === 1 }}>
            <For each={covers()}>{(url) => <img src={url} alt="" loading="lazy" decoding="async" draggable={false} />}</For>
          </div>
        }
      >
        <img class="col-cover" src={chosen()!.url} alt="" decoding="async" draggable={false} />
      </Show>
      <div class="col-shade" />
      <div class="col-label">
        <span class="col-kind">
          {KIND_LABEL[props.c.kind]}
          {props.c.hidden ? " · hidden" : ""}
        </span>
        <Emblem c={props.c} />
      </div>
    </div>
  );
}

/** The "New collection" tile (first in the Collections tab). */
export function NewCollectionTile(props: { focused: () => boolean }) {
  return (
    <div class="col-tile col-new" classList={{ focused: props.focused() }}>
      <span class="col-plus" aria-hidden="true">+</span>
      <span>New collection</span>
    </div>
  );
}

/** Asks for a name, then makes a collection (with a first game when given). */
export function newCollection(gameId: number | null, onMade?: (key: string) => void) {
  osk.open({
    title: "Name the new collection",
    value: "",
    onDone: (name) => {
      if (!name.trim()) return;
      void api
        .createCollection(name, gameId)
        .then(async (key) => {
          await collections.reload();
          onMade?.(key);
        })
        .catch(() => {});
    },
  });
}

/** The strip above an open collection's games. */
export function CollectionBanner(props: { c: Collection; onEdit: () => void; onBack: () => void }) {
  return (
    <div class="col-banner">
      <button class="btn ghost small" onClick={props.onBack} title="Back to Collections">
        ← Collections
      </button>
      <span class="col-kind">{KIND_LABEL[props.c.kind]}</span>
      <Emblem c={props.c} class="banner" />
      <button class="btn small" onClick={props.onEdit}>
        Edit…
      </button>
    </div>
  );
}

/**
 * Editing a collection or series: its logo (automatic, a game's logo, a
 * cover, or your own image), and for your own collections the name, the
 * games' order, its place in the list and deleting it; series can be hidden.
 */
export function CollectionEditor(props: { c: Collection; onClose: () => void; onDeleted?: () => void }) {
  const [picking, setPicking] = createSignal<"logo" | "cover" | null>(null);
  const [browsing, setBrowsing] = createSignal(false);
  const [confirmDelete, setConfirmDelete] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  // Always the latest version of this collection (after each change).
  const c = () => collections.byKey().get(props.c.key) ?? props.c;
  const members = () => collections.gamesOf(c());
  const mine = () => c().kind === "mine";

  async function run(f: () => Promise<unknown>) {
    setError(null);
    try {
      await f();
      await collections.reload();
    } catch (e) {
      setError(errorText(e));
    }
  }
  const setLogo = (choice: string) => run(() => api.setCollectionLogo(c().key, choice)).then(() => setPicking(null));
  const logoChoice = () => {
    const l = c().logo;
    return !l ? "auto" : l.type === "file" ? "file" : `${l.type}:${l.id}`;
  };

  return (
    <div class="overlay" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <NavScope class="panel col-editor" modal onBack={props.onClose}>
        <span class="col-kind">{KIND_LABEL[c().kind]}</span>
        <h2>{c().name}</h2>
        <div class="col-editor-preview">
          <CollectionTile c={c()} focused={() => false} />
        </div>

        <h3>Logo</h3>
        <div class="row">
          <button class="btn" classList={{ selected: logoChoice() === "auto" }} data-autofocus onClick={() => void setLogo("auto")}>
            Automatic
          </button>
          <button class="btn" classList={{ selected: logoChoice().startsWith("game:") }} onClick={() => setPicking(picking() === "logo" ? null : "logo")}>
            A game's logo…
          </button>
          <button class="btn" classList={{ selected: logoChoice().startsWith("cover:") }} onClick={() => setPicking(picking() === "cover" ? null : "cover")}>
            A cover…
          </button>
          <button class="btn" classList={{ selected: logoChoice() === "file" }} onClick={() => setBrowsing(true)}>
            Your image…
          </button>
        </div>
        <Show when={picking()}>
          {(kind) => {
            const options = () => members().filter((g) => (kind() === "logo" ? (g.logo ?? 0) > 0 : g.art > 0));
            return (
              <div class="col-choices">
                <Show when={options().length > 0} fallback={<p class="dim">None of its games has a {kind() === "logo" ? "logo" : "cover"} yet.</p>}>
                  <For each={options()}>
                    {(g) => (
                      <button
                        class="col-choice"
                        classList={{ logo: kind() === "logo", selected: logoChoice() === `${kind() === "logo" ? "game" : "cover"}:${g.id}` }}
                        title={g.title}
                        onClick={() => void setLogo(`${kind() === "logo" ? "game" : "cover"}:${g.id}`)}
                      >
                        <img src={kind() === "logo" ? artUrl(g.id, g.logo!, "l") : artUrl(g.id, g.art)} alt={g.title} decoding="async" />
                      </button>
                    )}
                  </For>
                </Show>
              </div>
            );
          }}
        </Show>
        <p class="dim small">
          Automatic shows a mosaic of its covers{mine() ? "" : " with the series' first logo"}. Your own image (PNG, JPEG or
          WebP) is copied into KB's folder.
        </p>

        <Show when={mine()}>
          <h3>Collection</h3>
          <div class="row">
            <button
              class="btn"
              onClick={() =>
                osk.open({ title: "Rename the collection", value: c().name, onDone: (name) => void run(() => api.renameCollection(c().key, name)) })
              }
            >
              Rename…
            </button>
            <button class="btn" onClick={() => void run(() => api.moveCollection(c().key, -1))}>
              Move earlier
            </button>
            <button class="btn" onClick={() => void run(() => api.moveCollection(c().key, 1))}>
              Move later
            </button>
          </div>
          <div class="row">
            <span class="dim">Order games by</span>
            <For each={[["added", "When added"], ["title", "Title"], ["release", "Release date"]] as const}>
              {([value, label]) => (
                <button class="btn small" classList={{ selected: c().gameOrder === value }} onClick={() => void run(() => api.setCollectionOrder(c().key, value))}>
                  {label}
                </button>
              )}
            </For>
          </div>
          <p class="dim small">Add or remove games from a game's details: Collections….</p>
          <div class="row">
            <Show
              when={confirmDelete()}
              fallback={
                <button class="btn danger" onClick={() => setConfirmDelete(true)}>
                  Delete collection…
                </button>
              }
            >
              <span>Delete “{c().name}”? Its games stay in your library.</span>
              <button
                class="btn danger"
                onClick={() =>
                  void run(() => api.deleteCollection(c().key)).then(() => {
                    props.onClose();
                    props.onDeleted?.();
                  })
                }
              >
                Delete
              </button>
              <button class="btn" onClick={() => setConfirmDelete(false)}>
                Keep it
              </button>
            </Show>
          </div>
        </Show>
        <Show when={!mine()}>
          <h3>Collections tab</h3>
          <div class="row">
            <button class="btn" onClick={() => void run(() => api.setCollectionHidden(c().key, !c().hidden))}>
              {c().hidden ? "Show in Collections" : "Hide from Collections"}
            </button>
          </div>
        </Show>

        <Show when={error()}>
          <p class="warn">{error()}</p>
        </Show>
        <div class="panel-actions">
          <div class="hints-row">
            <Hint action="select" label="Choose" />
            <Hint action="back" label="Done" />
          </div>
          <button class="btn" onClick={props.onClose}>
            Done
          </button>
        </div>
      </NavScope>
      <Show when={browsing()}>
        <FileBrowser
          title="Choose a logo image"
          mode="file"
          extensions={["png", "jpg", "jpeg", "webp"]}
          onPick={(path) => {
            setBrowsing(false);
            void run(() => api.setCollectionLogoFile(c().key, path)).then(() => setPicking(null));
          }}
          onClose={() => setBrowsing(false)}
        />
      </Show>
    </div>
  );
}

/** From a game's details: tick the collections it's in, or make a new one with it. */
export function CollectionPicker(props: { gameId: number; title: string; onClose: () => void }) {
  const [error, setError] = createSignal<string | null>(null);
  const isIn = (c: Collection) => c.games.includes(props.gameId);
  async function toggle(c: Collection) {
    setError(null);
    try {
      await api.setInCollection(c.key, props.gameId, !isIn(c));
      await collections.reload();
    } catch (e) {
      setError(errorText(e));
    }
  }
  // The series it's part of (automatic, for information).
  const series = () => collections.list().filter((c) => c.kind !== "mine" && isIn(c));

  return (
    <div class="overlay" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <NavScope class="panel col-picker" modal onBack={props.onClose}>
        <h2>Collections</h2>
        <p class="dim">{props.title}</p>
        <div class="col-picker-list">
          <For each={collections.mine()}>
            {(c) => (
              <label class="check">
                <input type="checkbox" checked={isIn(c)} onChange={() => void toggle(c)} />
                {c.name}
              </label>
            )}
          </For>
          <Show when={collections.mine().length === 0}>
            <p class="dim">You haven't made a collection yet.</p>
          </Show>
        </div>
        <div class="row">
          <button class="btn" data-autofocus={collections.mine().length === 0 ? "" : undefined} onClick={() => newCollection(props.gameId)}>
            New collection with this game…
          </button>
        </div>
        <Show when={series().length > 0}>
          <p class="dim small">Also in {series().map((s) => s.name).join(", ")} (from its series, automatically).</p>
        </Show>
        <Show when={error()}>
          <p class="warn">{error()}</p>
        </Show>
        <div class="panel-actions">
          <div class="hints-row">
            <Hint action="select" label="Add / remove" />
            <Hint action="back" label="Done" />
          </div>
          <button class="btn" onClick={props.onClose}>
            Done
          </button>
        </div>
      </NavScope>
    </div>
  );
}

/** Opens a collection from the overview (used by tiles and the banner). */
export const openCollection = (key: string) => library.setPlatform(`col:${key}`);
