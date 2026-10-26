import { createMemo, createRoot, For, Show, type JSX } from "solid-js";
import { device, type Action } from "../input/actions";
import { session } from "../state/session";

const PS: Partial<Record<Action, string>> = {
  select: "✕", back: "○", secondary: "□", favorite: "△",
  prevPlatform: "L1", nextPlatform: "R1", prevLetter: "L2", nextLetter: "R2", menu: "Options",
  zoomIn: "R↑", zoomOut: "R↓", toggleGroups: "R3", filter: "Share", toggleTv: "L3", turnLeft: "R←", turnRight: "R→",
};
const XBOX: Partial<Record<Action, string>> = {
  select: "A", back: "B", secondary: "X", favorite: "Y",
  prevPlatform: "LB", nextPlatform: "RB", prevLetter: "LT", nextLetter: "RT", menu: "Menu",
  zoomIn: "R↑", zoomOut: "R↓", toggleGroups: "RS", filter: "View", toggleTv: "LS", turnLeft: "R←", turnRight: "R→",
};
const KEYS: Partial<Record<Action, string>> = {
  select: "Enter", back: "Esc", secondary: "/", favorite: "F",
  prevPlatform: "Q", nextPlatform: "E", prevLetter: "Home", nextLetter: "End", menu: "Esc",
  zoomIn: "+", zoomOut: "−", toggleGroups: "V", filter: "G", toggleTv: "F10", turnLeft: "[", turnRight: "]",
};

const FACE: Action[] = ["select", "back", "secondary", "favorite"];

export function glyph(a: Action, pad = device() === "gamepad"): string {
  if (pad) {
    const map = session.lastPad()?.family === "playstation" ? PS : XBOX;
    return map[a] ?? a;
  }
  return KEYS[a] ?? a;
}

// PlayStation symbols drawn as shapes: the font's ○ □ △ are thin and sit low.
// Functions, not elements: a DOM node can only be in one place, so each glyph needs its own.
const PS_SHAPE: Partial<Record<Action, () => JSX.Element>> = {
  select: () => <path d="M7 7l10 10M17 7L7 17" />,
  back: () => <circle cx="12" cy="12" r="6" />,
  secondary: () => <rect x="6.5" y="6.5" width="11" height="11" />,
  favorite: () => <path d="M12 5.5l6.5 11.5h-13z" />,
};

function psShape(a: Action) {
  return (
    <svg viewBox="0 0 24 24" width="1.1em" height="1.1em" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linejoin="round" stroke-linecap="round" aria-label={glyph(a, true)}>
      {PS_SHAPE[a]!()}
    </svg>
  );
}

// The last keyboard-or-pad input decides; a stray mouse move doesn't hide the glyphs.
export const padShown = createRoot(() => {
  let last = device();
  return createMemo(() => {
    const d = device();
    if (d !== "pointer") last = d;
    return last === "gamepad";
  });
});

/** A glyph with no label, placed on the control it operates; shown only after controller input. */
export function Badge(props: { action?: Action; actions?: Action[] }) {
  return (
    <Show when={padShown()}>
      <Hint {...props} pad />
    </Show>
  );
}

/** One hint: the button glyph(s) for the current device (or always the pad's, with `pad`) plus a label. */
export function Hint(props: { action?: Action; actions?: Action[]; label?: string; pad?: boolean }) {
  const actions = () => props.actions ?? (props.action ? [props.action] : []);
  const pad = () => !!props.pad || device() === "gamepad";
  const ps = () => pad() && session.lastPad()?.family === "playstation";
  return (
    <span class="hint">
      <For each={actions()}>
        {(a, i) => (
          <>
            {i() > 0 && <span class="hint-sep">/</span>}
            <kbd classList={{ face: pad() && FACE.includes(a), pad: pad() && !FACE.includes(a), ps: ps(), [`f-${a}`]: pad() }}>
              {ps() && PS_SHAPE[a] ? psShape(a) : glyph(a, pad())}
            </kbd>
          </>
        )}
      </For>
      <Show when={props.label}><span class="hint-label">{props.label}</span></Show>
    </span>
  );
}
