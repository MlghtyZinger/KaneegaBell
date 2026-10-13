import { For } from "solid-js";
import { device, type Action } from "../input/actions";
import { session } from "../state/session";

const PS: Partial<Record<Action, string>> = {
  select: "✕", back: "○", secondary: "□", favorite: "△",
  prevPlatform: "L1", nextPlatform: "R1", prevLetter: "L2", nextLetter: "R2", menu: "Options",
  zoomIn: "R↑", zoomOut: "R↓", toggleLayout: "R3",
};
const XBOX: Partial<Record<Action, string>> = {
  select: "A", back: "B", secondary: "X", favorite: "Y",
  prevPlatform: "LB", nextPlatform: "RB", prevLetter: "LT", nextLetter: "RT", menu: "Menu",
  zoomIn: "R↑", zoomOut: "R↓", toggleLayout: "RS",
};
const KEYS: Partial<Record<Action, string>> = {
  select: "Enter", back: "Esc", secondary: "/", favorite: "F",
  prevPlatform: "Q", nextPlatform: "E", prevLetter: "Home", nextLetter: "End", menu: "F1",
  zoomIn: "+", zoomOut: "−", toggleLayout: "V",
};

export function glyph(a: Action): string {
  if (device() === "gamepad") {
    const map = session.lastPad()?.family === "playstation" ? PS : XBOX;
    return map[a] ?? a;
  }
  return KEYS[a] ?? a;
}

/** One hint: the button glyph(s) for the current device plus a label. */
export function Hint(props: { action?: Action; actions?: Action[]; label: string }) {
  const actions = () => props.actions ?? (props.action ? [props.action] : []);
  const ps = () => device() === "gamepad" && session.lastPad()?.family === "playstation";
  return (
    <span class="hint">
      <For each={actions()}>
        {(a, i) => (
          <>
            {i() > 0 && <span class="hint-sep">/</span>}
            <kbd classList={{ "ps-glyph": ps() && ["select", "back", "secondary", "favorite"].includes(a), [`ps-${a}`]: ps() }}>
              {glyph(a)}
            </kbd>
          </>
        )}
      </For>
      <span class="hint-label">{props.label}</span>
    </span>
  );
}
