import { createMemo, createSignal, For, Show } from "solid-js";
import { api, artUrl, errorText, type Collection, type CollectionRules, type CoverCandidate, type GameRow } from "../api";
import { collections } from "../state/collections";
import { library } from "../state/library";
import { osk } from "../state/osk";
import { NavScope } from "./NavScope";
import { FileBrowser } from "./FileBrowser";
import { FilterPanel } from "./FilterPanel";
import { Hint } from "./Hints";
import { Icon } from "./Icons";

const KIND_LABEL = { mine: "Collection", series: "Series", franchise: "Franchise", smart: "Smart" } as const;

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

/**
 * A tile in the Collections tab: a cover mosaic (or the chosen cover) with its
 * logo or name. `onEdit` adds a pencil (shown on hover and focus).
 */
export function CollectionTile(props: { c: Collection; focused: () => boolean; onEdit?: () => void }) {
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
      <Show when={props.onEdit}>
        <button
          class="col-edit"
          title={props.c.kind === "mine" ? "Rename, logo and more (Y)" : props.c.kind === "smart" ? "Rename, rules, logo and more (Y)" : "Logo, hide (Y)"}
          tabIndex={-1}
          onClick={(e) => {
            e.stopPropagation();
            props.onEdit!();
          }}
        >
          <Icon name="pencil" />
        </button>
      </Show>
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

/** Asks for a name, then makes a smart collection whose games follow `rules`. */
export function saveSmartCollection(rules: CollectionRules, onMade?: (key: string) => void) {
  osk.open({
    title: "Name the smart collection",
    value: "",
    onDone: (name) => {
      if (!name.trim()) return;
      void api
        .createSmartCollection(name, rules)
        .then(async (key) => {
          await collections.reload();
          onMade?.(key);
        })
        .catch(() => {});
    },
  });
}

/** Asks for a new name for one of the user's collections. */
export function renameCollection(c: Collection) {
  osk.open({
    title: "Rename the collection",
    value: c.name,
    onDone: (name) => {
      if (!name.trim() || name.trim() === c.name) return;
      void api
        .renameCollection(c.key, name)
        .then(() => collections.reload())
        .catch(() => {});
    },
  });
}

/** The banner's buttons, in order (for controller focus). */
export const bannerActions = (c: Collection) =>
  c.kind === "mine" || c.kind === "smart" ? (["back", "rename", "edit"] as const) : (["back", "edit"] as const);

/**
 * The strip above an open collection's games. `focused` is the controller's
 * place among its buttons (up from the top row of covers), or null.
 */
export function CollectionBanner(props: { c: Collection; focused: string | null; onEdit: () => void; onBack: () => void }) {
  return (
    <div class="col-banner">
      <button class="btn ghost small" classList={{ "nav-focus": props.focused === "back" }} onClick={props.onBack} title="Back to Collections">
        ← Collections
      </button>
      <span class="col-kind">{KIND_LABEL[props.c.kind]}</span>
      <Emblem c={props.c} class="in-banner" />
      <Show when={props.c.kind === "mine"}>
        <button class="btn small" classList={{ "nav-focus": props.focused === "rename" }} onClick={() => renameCollection(props.c)}>
          Rename…
        </button>
      </Show>
      <button class="btn small" classList={{ "nav-focus": props.focused === "edit" }} onClick={props.onEdit}>
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
  const [picking, setPicking] = createSignal<"logo" | "cover" | "search" | null>(null);
  const [query, setQuery] = createSignal("");
  const [found, setFound] = createSignal<CoverCandidate[] | null>(null);
  const [searching, setSearching] = createSignal(false);
  const [browsing, setBrowsing] = createSignal(false);
  const [confirmDelete, setConfirmDelete] = createSignal(false);
  const [editingRules, setEditingRules] = createSignal(false);
  const [managingMembers, setManagingMembers] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  // Always the latest version of this collection (after each change).
  const c = () => collections.byKey().get(props.c.key) ?? props.c;
  const members = () => collections.gamesOf(c());
  const mine = () => c().kind === "mine";
  const smart = () => c().kind === "smart";

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
  async function searchLogos() {
    setError(null);
    setSearching(true);
    setFound(null);
    try {
      setFound(await api.collectionLogoCandidates(query().trim()));
    } catch (e) {
      setError(errorText(e));
    }
    setSearching(false);
  }
  const openSearch = () => {
    if (picking() === "search") return setPicking(null);
    setQuery(c().name);
    setFound(null);
    setPicking("search");
    void searchLogos();
  };
  const pickFound = (url: string) => run(() => api.setCollectionLogoUrl(c().key, url)).then(() => setPicking(null));
  const logoChoice = () => {
    const l = c().logo;
    return !l ? "auto" : l.type === "file" ? "file" : `${l.type}:${l.id}`;
  };

  return (
    <div class="overlay" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <NavScope class="panel col-editor" modal onBack={props.onClose}>
        <span class="col-kind">{KIND_LABEL[c().kind]}</span>
        <h2>{c().name}</h2>
        <Show when={collections.statsLine(c())}>
          <p class="dim small">{collections.statsLine(c())}</p>
        </Show>
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
          <button class="btn" classList={{ selected: picking() === "search" }} onClick={openSearch}>
            Search logos…
          </button>
        </div>
        <Show when={picking() === "search"}>
          <div class="row">
            <input
              class="input"
              value={query()}
              placeholder="Search SteamGridDB"
              onInput={(e) => setQuery(e.currentTarget.value)}
              onKeyDown={(e) => e.key === "Enter" && void searchLogos()}
            />
            <button class="btn" onClick={() => void searchLogos()}>
              Search
            </button>
          </div>
          <Show when={searching()}>
            <p class="dim">Searching SteamGridDB…</p>
          </Show>
          <Show when={found()}>
            {(list) => (
              <div class="col-choices">
                <Show when={list().length > 0} fallback={<p class="dim">No logos found.</p>}>
                  <For each={list()}>
                    {(r) => (
                      <button class="col-choice logo" title={r.label} onClick={() => void pickFound(r.url)}>
                        <img src={r.preview ?? r.url} alt={r.label} loading="lazy" decoding="async" />
                      </button>
                    )}
                  </For>
                </Show>
              </div>
            )}
          </Show>
        </Show>
        <Show when={picking() === "logo" ? "logo" : picking() === "cover" ? "cover" : null}>
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
          Automatic shows a mosaic of its covers{mine() || smart() ? "" : " with the series' first logo"}. Your own image (PNG, JPEG or
          WebP) is copied into KB's folder. Searching logos needs your SteamGridDB key (Settings → Game info).
        </p>

        <Show when={mine() || smart()}>
          <h3>{smart() ? "Smart collection" : "Collection"}</h3>
          <div class="row">
            <button class="btn" onClick={() => renameCollection(c())}>
              Rename…
            </button>
            <button class="btn" onClick={() => void run(() => api.moveCollection(c().key, -1))}>
              Move earlier
            </button>
            <button class="btn" onClick={() => void run(() => api.moveCollection(c().key, 1))}>
              Move later
            </button>
          </div>
          <Show when={smart()}>
            <div class="row">
              <button class="btn" onClick={() => setEditingRules(true)}>
                Edit rules…
              </button>
            </div>
          </Show>
          <Show when={mine()}>
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
          </Show>
          <Show when={mine()}>
            <div class="row">
              <button class="btn" onClick={() => setManagingMembers(true)}>Add games…</button>
            </div>
            <p class="dim small">Tick games to add them. On the collection page, press Y or right-click a game to remove it.</p>
          </Show>
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
        <Show when={!mine() && !smart()}>
          <h3>Collections tab</h3>
          <div class="row">
            <button class="btn" onClick={() => void run(() => api.setCollectionHidden(c().key, !c().hidden))}>
              {c().hidden ? "Show in Collections" : "Hide from Collections"}
            </button>
            <button class="btn" onClick={() => setManagingMembers(true)}>Add games…</button>
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
      <Show when={editingRules()}>
        <FilterPanel
          initial={c().rules ?? {}}
          onSave={(r) => {
            void run(() => api.setCollectionRules(c().key, r)).then(() => setEditingRules(false));
          }}
          onClose={() => setEditingRules(false)}
        />
      </Show>
      <Show when={managingMembers()}>
        <CollectionMembersPicker
          c={c()}
          onClose={() => setManagingMembers(false)}
        />
      </Show>
    </div>
  );
}

/** Searchable, tick-to-toggle membership editor for a manual collection or series. */
function CollectionMembersPicker(props: { c: Collection; onClose: () => void }) {
  const [query, setQuery] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const collection = () => collections.byKey().get(props.c.key) ?? props.c;
  const selected = createMemo(() => new Set(collections.gamesOf(collection()).map((g) => g.id)));
  const matching = createMemo(() => {
    const q = query().trim().toLocaleLowerCase();
    const found = library.games().filter((g) => !q || `${g.title} ${g.variant ?? ""} ${g.platformId}`.toLocaleLowerCase().includes(q));
    return { total: found.length, games: found.slice(0, 120) };
  });

  async function toggle(game: GameRow, member: boolean) {
    if (busy()) return;
    setBusy(true);
    setError(null);
    try {
      await api.setInCollection(props.c.key, game.id, member);
      await collections.reload();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div class="overlay" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <NavScope class="panel col-picker col-members-picker" modal onBack={props.onClose}>
        <h2>Add games</h2>
        <p class="dim">{collection().name} · tick or untick games to change its members.</p>
        <input
          class="input"
          data-autofocus
          type="search"
          placeholder="Search games…"
          value={query()}
          onInput={(e) => setQuery(e.currentTarget.value)}
        />
        <p class="dim small">
          {matching().total > matching().games.length ? `Showing ${matching().games.length} of ${matching().total}; search to narrow the list.` : `${matching().total} games`}
        </p>
        <div class="col-picker-list">
          <For each={matching().games}>
            {(g) => (
              <label class="col-member" classList={{ selected: selected().has(g.id) }}>
                <input type="checkbox" checked={selected().has(g.id)} disabled={busy()} onChange={(e) => void toggle(g, e.currentTarget.checked)} />
                <span>{g.title}{g.variant ? ` · ${g.variant}` : ""}</span>
                <small>{library.platforms().find((p) => p.id === g.platformId)?.shortName ?? g.platformId}</small>
              </label>
            )}
          </For>
          <Show when={matching().games.length === 0}>
            <p class="dim">No games match that search.</p>
          </Show>
        </div>
        <Show when={error()}><p class="warn">{error()}</p></Show>
        <div class="panel-actions">
          <div class="hints-row"><Hint action="select" label="Choose" /><Hint action="back" label="Done" /></div>
          <button class="btn" onClick={props.onClose}>Done</button>
        </div>
      </NavScope>
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
