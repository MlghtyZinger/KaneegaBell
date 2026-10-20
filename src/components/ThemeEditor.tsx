import { createEffect, createResource, createSignal, For, on, onCleanup, Show } from "solid-js";
import { createStore, produce, unwrap } from "solid-js/store";
import { api, errorText, type PictureChange, type ThemeData } from "../api";
import { applyTheme, FOCUS_LOOKS } from "../state/theme";
import { FileBrowser } from "./FileBrowser";
import { Hint } from "./Hints";
import { NavScope } from "./NavScope";
import { Picker } from "./Picker";

/** What each color is used for, in the editor's order. */
const COLORS: [string, string][] = [
  ["bg", "Window background"],
  ["bg-raised", "Panels and menus"],
  ["bg-sunken", "Platform list, TV Mode shading"],
  ["surface", "Buttons"],
  ["surface-hover", "Buttons, hovered"],
  ["border", "Lines"],
  ["text", "Text"],
  ["text-dim", "Secondary text"],
  ["text-faint", "Hints"],
  ["accent", "Accent (selected tab, Play)"],
  ["accent-strong", "Accent text"],
  ["on-accent", "Text on the accent"],
  ["focus", "Focus ring"],
  ["gold", "Favorites, achievements"],
  ["danger", "Errors"],
  ["ok", "Success"],
];

const ICONS = ["ember", "midnight", "daylight", "contrast", "moss", "storm"].map((id) => ({ value: id, label: id[0].toUpperCase() + id.slice(1) }));

function hsl(h: number, s: number, l: number): string {
  const a = (s / 100) * Math.min(l / 100, 1 - l / 100);
  const f = (n: number) => {
    const k = (n + h / 30) % 12;
    return Math.round(255 * (l / 100 - a * Math.max(-1, Math.min(k - 3, 9 - k, 1))))
      .toString(16)
      .padStart(2, "0");
  };
  return `#${f(0)}${f(8)}${f(4)}`;
}

/** 12 hues x 8 lightnesses, twice (vivid, then muted), and a row of grays: a grid the D-pad can walk. */
const PALETTE: string[][] = [
  ...[72, 28].flatMap((s) =>
    [10, 20, 32, 45, 58, 70, 82, 92].map((l) => Array.from({ length: 12 }, (_, i) => hsl(i * 30, s, l))),
  ),
  Array.from({ length: 12 }, (_, i) => hsl(0, 0, Math.round(3 + (i * 94) / 11))),
];

function ColorPicker(props: { title: string; value: string; onPick: (hex: string) => void; onClose: () => void }) {
  const [hex, setHex] = createSignal(props.value);
  const valid = () => /^#[0-9a-f]{6}$/i.test(hex());
  return (
    <div class="overlay" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <NavScope class="panel color-picker" modal onBack={props.onClose}>
        <h3>{props.title}</h3>
        <div class="palette">
          <For each={PALETTE.flat()}>
            {(c) => (
              <button
                class="swatch"
                classList={{ selected: c === props.value.toLowerCase() }}
                style={{ background: c }}
                aria-label={c}
                data-autofocus={c === props.value.toLowerCase() ? "" : undefined}
                onClick={() => props.onPick(c)}
              />
            )}
          </For>
        </div>
        <div class="row">
          <input
            class="input mono"
            aria-label="Hex color"
            value={hex()}
            maxLength={7}
            onInput={(e) => setHex(e.currentTarget.value.trim())}
          />
          <button class="btn" disabled={!valid()} onClick={() => props.onPick(hex().toLowerCase())}>
            Use this
          </button>
        </div>
        <div class="panel-actions">
          <div class="hints-row">
            <Hint action="select" label="Choose" />
            <Hint action="back" label="Back" />
          </div>
        </div>
      </NavScope>
    </div>
  );
}

function Stepper(props: { label: string; value: number; min: number; max: number; step: number; unit?: string; onChange: (v: number) => void }) {
  const set = (v: number) => props.onChange(Math.max(props.min, Math.min(props.max, v)));
  return (
    <div class="row stepper">
      <button class="btn" aria-label={`Less ${props.label}`} disabled={props.value <= props.min} onClick={() => set(props.value - props.step)}>
        −
      </button>
      <span class="stepper-value">
        {props.value}
        {props.unit}
      </span>
      <button class="btn" aria-label={`More ${props.label}`} disabled={props.value >= props.max} onClick={() => set(props.value + props.step)}>
        +
      </button>
    </div>
  );
}

/**
 * Makes or edits one of the user's styles. The app itself is the live preview: every change
 * (after a short pause) goes through the same checks as an import and, when it passes, is
 * applied to the whole window. Cancel puts the style that was in use back.
 */
export function ThemeEditor(props: {
  /** The style to start from. */
  baseId: string;
  /** The user's style being changed, or null for a new one. */
  editId: string | null;
  /** The style in use when the editor opened, restored on cancel. */
  originalId: string;
  onClose: (savedId?: string) => void;
}) {
  const [base] = createResource(() => api.getTheme(props.baseId));
  return <Show when={base()}>{(b) => <Editor {...props} base={b()} />}</Show>;
}

function Editor(props: { baseId: string; editId: string | null; originalId: string; base: ThemeData; onClose: (savedId?: string) => void }) {
  const fresh = !props.editId;
  const start = structuredClone(props.base);
  if (fresh) {
    start.name = props.base.name.startsWith("My ") ? props.base.name : `My ${props.base.name}`;
    start.author = "Me";
    start.background = undefined;
  }
  const [theme, setTheme] = createStore<ThemeData>(start);
  const [picture, setPicture] = createSignal<PictureChange>({ kind: "keep" });
  const [problems, setProblems] = createSignal<string[]>([]);
  const [error, setError] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [picking, setPicking] = createSignal<string | null>(null);
  const [browsing, setBrowsing] = createSignal(false);

  const hasPicture = () => {
    const p = picture();
    return p.kind === "set" || (p.kind === "keep" && !!theme.background);
  };

  // Check and preview after a pause, dropping answers that a newer change has overtaken.
  let seq = 0;
  let timer: number | undefined;
  createEffect(
    on(
      () => [JSON.stringify(theme), JSON.stringify(picture())],
      () => {
        clearTimeout(timer);
        timer = window.setTimeout(async () => {
          const mine = ++seq;
          try {
            const r = await api.checkTheme(props.editId, unwrap(theme), picture());
            if (mine !== seq) return;
            setProblems(r.problems);
            if (!r.problems.length) applyTheme({ id: "preview", css: r.css, focusLook: theme.focusLook, icon: theme.icon });
          } catch (e) {
            if (mine === seq) setProblems([errorText(e)]);
          }
        }, 200);
      },
    ),
  );
  onCleanup(() => clearTimeout(timer));

  const cancel = async () => {
    try {
      applyTheme(await api.setTheme(props.originalId));
    } catch {
      /* the page keeps the preview; a restart restores the saved style */
    }
    props.onClose();
  };

  const save = async () => {
    setBusy(true);
    setError("");
    try {
      const data = unwrap(theme);
      if (!hasPicture()) data.background = undefined;
      const id = await api.saveTheme(props.editId, data, picture());
      applyTheme(await api.setTheme(id));
      props.onClose(id);
    } catch (e) {
      setError(errorText(e));
      setBusy(false);
    }
  };

  return (
    <div class="overlay theme-editor-overlay">
      <NavScope class="panel theme-editor" modal onBack={() => void cancel()}>
        <h2>{fresh ? "New style" : "Edit style"}</h2>
        <div class="form-grid">
          <label for="te-name">Name</label>
          <input id="te-name" class="input" value={theme.name} maxLength={40} onInput={(e) => setTheme("name", e.currentTarget.value)} />
          <label for="te-author">Author</label>
          <input id="te-author" class="input" value={theme.author} maxLength={40} onInput={(e) => setTheme("author", e.currentTarget.value)} />
          <span>Look</span>
          <Picker
            title="Dark or light"
            value={theme.scheme}
            options={[
              { value: "dark", label: "Dark" },
              { value: "light", label: "Light" },
            ]}
            onChange={(v) => setTheme("scheme", v)}
          />
          <span>Focus marker</span>
          <Picker
            title="How the focused game is marked"
            value={theme.focusLook}
            options={Object.entries(FOCUS_LOOKS).map(([value, label]) => ({ value, label }))}
            onChange={(v) => setTheme("focusLook", v)}
          />
          <span>Icon colors</span>
          <Picker title="Window icon" value={theme.icon} options={ICONS} onChange={(v) => setTheme("icon", v)} />
          <span>Corners</span>
          <Stepper label="rounding" value={theme.radius} min={0} max={24} step={2} unit=" px" onChange={(v) => setTheme("radius", v)} />
        </div>

        <h3>Colors</h3>
        <div class="color-list">
          <For each={COLORS}>
            {([key, label]) => (
              <button class="color-row" onClick={() => setPicking(key)}>
                <span class="swatch small" style={{ background: theme.colors[key] }} />
                <span class="color-name">{label}</span>
                <span class="mono dim">{theme.colors[key]}</span>
              </button>
            )}
          </For>
        </div>

        <h3>Background picture</h3>
        <div class="row">
          <button class="btn" onClick={() => setBrowsing(true)}>
            {hasPicture() ? "Change picture…" : "Choose picture…"}
          </button>
          <Show when={hasPicture()}>
            <button
              class="btn"
              onClick={() => {
                setPicture({ kind: "remove" });
                setTheme("background", undefined);
              }}
            >
              Remove
            </button>
            <span class="dim">Dim</span>
            <Stepper
              label="dimming"
              value={theme.background?.dim ?? 50}
              min={0}
              max={90}
              step={10}
              unit="%"
              onChange={(v) => setTheme("background", { dim: v })}
            />
          </Show>
        </div>
        <p class="dim small">
          The picture sits behind the window; panels that aren't see-through cover it. It is resized and re-encoded, and stored with the style.
        </p>

        <Show when={problems().length || error()}>
          <ul class="problems">
            <For each={[...problems(), ...(error() ? [error()] : [])]}>{(p) => <li>{p}</li>}</For>
          </ul>
        </Show>

        <div class="panel-actions">
          <div class="hints-row">
            <Hint action="select" label="Choose" />
            <Hint action="back" label="Cancel" />
          </div>
          <button class="btn" onClick={() => void cancel()}>
            Cancel
          </button>
          <button class="btn primary" disabled={busy() || problems().length > 0 || !theme.name.trim() || !theme.author.trim()} onClick={() => void save()}>
            Save
          </button>
        </div>

        <Show when={picking()}>
          {(key) => (
            <ColorPicker
              title={COLORS.find(([k]) => k === key())![1]}
              value={theme.colors[key()]}
              onPick={(hex) => {
                setTheme(produce((t) => void (t.colors[key()] = hex)));
                setPicking(null);
              }}
              onClose={() => setPicking(null)}
            />
          )}
        </Show>
        <Show when={browsing()}>
          <FileBrowser
            title="Choose a background picture"
            mode="file"
            extensions={["png", "jpg", "jpeg", "webp"]}
            onPick={(path) => {
              setPicture({ kind: "set", path });
              setTheme("background", { dim: theme.background?.dim ?? 50 });
              setBrowsing(false);
            }}
            onClose={() => setBrowsing(false)}
          />
        </Show>
      </NavScope>
    </div>
  );
}
