import { createMemo, createSignal, For, Show } from "solid-js";
import type { CollectionRules } from "../api";
import { library } from "../state/library";
import { emptyRules } from "../state/smartFilter";
import { NavScope } from "./NavScope";
import { Picker } from "./Picker";
import { Hint } from "./Hints";

const PLAY_BUCKETS: [string, number | null][] = [
  ["Any", null],
  ["1 hour", 3600],
  ["5 hours", 5 * 3600],
  ["10 hours", 10 * 3600],
  ["20 hours", 20 * 3600],
  ["50 hours", 50 * 3600],
];

/** Toggle a value in a multi-select list, returning a new list (undefined when empty). */
function toggle<T>(list: T[] | undefined, value: T, on: boolean): T[] | undefined {
  const next = new Set(list ?? []);
  if (on) next.add(value);
  else next.delete(value);
  return next.size ? [...next] : undefined;
}

/** A "choose several" control: a button that opens a checklist. */
function MultiList(props: { title: string; values: string[]; selected: string[] | undefined; onToggle: (v: string, on: boolean) => void }) {
  const [open, setOpen] = createSignal(false);
  const isOn = (v: string) => props.selected?.includes(v) ?? false;
  return (
    <>
      <button class="btn small" classList={{ selected: (props.selected?.length ?? 0) > 0 }} onClick={() => setOpen(true)}>
        {props.title}
        {props.selected?.length ? ` (${props.selected.length})` : ""}
      </button>
      <Show when={open()}>
        <div class="overlay" onClick={(e) => e.target === e.currentTarget && setOpen(false)}>
          <NavScope class="panel picker-panel" modal onBack={() => setOpen(false)}>
            <h3>{props.title}</h3>
            <div class="picker-list">
              <For each={props.values}>
                {(v) => (
                  <button class="picker-option" classList={{ selected: isOn(v) }} onClick={() => props.onToggle(v, !isOn(v))}>
                    <span class="check">{isOn(v) ? "✓ " : ""}</span>
                    {v}
                  </button>
                )}
              </For>
            </div>
          </NavScope>
        </div>
      </Show>
    </>
  );
}

/**
 * The saved-filter panel: platform / genre / developer / publisher / year /
 * played / favorite / play time. `onChange` fires live (browse mode); `onSave`
 * or `onSaveAs` add the save action (edit mode vs. browse mode).
 */
export function FilterPanel(props: {
  initial: CollectionRules;
  onClose: () => void;
  onChange?: (rules: CollectionRules) => void;
  onSave?: (rules: CollectionRules) => void;
  onSaveAs?: (rules: CollectionRules) => void;
}) {
  const [rules, setRules] = createSignal<CollectionRules>({ ...props.initial });
  const set = (patch: Partial<CollectionRules>) => {
    const next = { ...rules(), ...patch };
    setRules(next);
    props.onChange?.(next);
  };
  const clear = () => {
    setRules(emptyRules());
    props.onChange?.(emptyRules());
  };

  const years = createMemo(() =>
    [...new Set(library.games().map((g) => g.year).filter((y): y is number => y != null))].sort((a, b) => a - b),
  );
  const yearOptions = () => [{ value: "", label: "Any year" }, ...years().map((y) => ({ value: String(y), label: String(y) }))];
  const playOptions = () => PLAY_BUCKETS.map(([label, secs]) => ({ value: secs === null ? "" : String(secs), label }));

  const has = (list: string[] | undefined, v: string) => list?.includes(v) ?? false;
  const hasGenre = (id: number) => rules().genres?.includes(id) ?? false;
  const validRanges = () => {
    const r = rules();
    return (r.yearFrom == null || r.yearTo == null || r.yearFrom <= r.yearTo) &&
      (r.playFrom == null || r.playTo == null || r.playFrom <= r.playTo);
  };

  return (
    <div class="overlay" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <NavScope class="panel filter-panel" modal onBack={props.onClose}>
        <h2>Filter</h2>

        <h3>Platform</h3>
        <div class="filter-chips">
          <For each={library.platforms()}>
            {(p) => (
              <button class="btn small" classList={{ selected: has(rules().platforms, p.id) }} onClick={() => set({ platforms: toggle(rules().platforms, p.id, !has(rules().platforms, p.id)) })}>
                {p.name}
              </button>
            )}
          </For>
        </div>

        <Show when={library.genres().length > 0}>
          <h3>Genre</h3>
          <div class="filter-chips">
            <For each={library.genres()}>
              {(g) => (
                <button class="btn small" classList={{ selected: hasGenre(g.id) }} onClick={() => set({ genres: toggle(rules().genres, g.id, !hasGenre(g.id)) })}>
                  {g.name} ({library.genreCounts().get(g.id) ?? 0})
                </button>
              )}
            </For>
          </div>
        </Show>

        <h3>Who made it</h3>
        <div class="row">
          <MultiList title="Developer" values={library.developers()} selected={rules().developers} onToggle={(v, on) => set({ developers: toggle(rules().developers, v, on) })} />
          <MultiList title="Publisher" values={library.publishers()} selected={rules().publishers} onToggle={(v, on) => set({ publishers: toggle(rules().publishers, v, on) })} />
        </div>

        <h3>Year</h3>
        <div class="row">
          <span class="dim">From</span>
          <Picker value={rules().yearFrom == null ? "" : String(rules().yearFrom)} options={yearOptions()} onChange={(v) => set({ yearFrom: v === "" ? undefined : Number(v) })} />
          <span class="dim">To</span>
          <Picker value={rules().yearTo == null ? "" : String(rules().yearTo)} options={yearOptions()} onChange={(v) => set({ yearTo: v === "" ? undefined : Number(v) })} />
        </div>

        <h3>Played</h3>
        <div class="filter-chips">
          <button class="btn small" classList={{ selected: !rules().played }} onClick={() => set({ played: undefined })}>
            Any
          </button>
          <button class="btn small" classList={{ selected: rules().played === "played" }} onClick={() => set({ played: "played" })}>
            Played
          </button>
          <button class="btn small" classList={{ selected: rules().played === "unplayed" }} onClick={() => set({ played: "unplayed" })}>
            Unplayed
          </button>
        </div>

        <h3>Play time</h3>
        <div class="row">
          <span class="dim">At least</span>
          <Picker value={rules().playFrom == null ? "" : String(rules().playFrom)} options={playOptions()} onChange={(v) => set({ playFrom: v === "" ? undefined : Number(v) })} />
          <span class="dim">At most</span>
          <Picker value={rules().playTo == null ? "" : String(rules().playTo)} options={playOptions()} onChange={(v) => set({ playTo: v === "" ? undefined : Number(v) })} />
        </div>

        <h3>Favorite</h3>
        <div class="filter-chips">
          <button class="btn small" classList={{ selected: rules().favorite === true }} onClick={() => set({ favorite: rules().favorite !== true })}>
            Favorite only
          </button>
        </div>

        <Show when={!validRanges()}>
          <p class="error">Each “From” value must be no greater than its “To” value.</p>
        </Show>

        <div class="panel-actions">
          <div class="hints-row">
            <Hint action="select" label="Choose" />
            <Hint action="back" label="Done" />
          </div>
          <button class="btn" onClick={clear}>
            Clear
          </button>
          <Show when={props.onSave}>
            <button class="btn primary" disabled={!validRanges()} onClick={() => props.onSave!(rules())}>
              Save rules
            </button>
          </Show>
          <Show when={props.onSaveAs}>
            <button class="btn primary" disabled={!validRanges()} onClick={() => props.onSaveAs!(rules())}>
              Save as collection…
            </button>
          </Show>
          <button class="btn" onClick={props.onClose}>
            Done
          </button>
        </div>
      </NavScope>
    </div>
  );
}
