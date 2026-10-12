// Abstract UI actions. Keyboard maps here today; the Rust gamepad thread will
// emit the same actions in milestone c, so views never care about the device.

export type Action =
  | "up"
  | "down"
  | "left"
  | "right"
  | "select"
  | "back"
  | "prevPlatform"
  | "nextPlatform"
  | "prevLetter"
  | "nextLetter"
  | "favorite"
  | "menu";

const KEYMAP: Record<string, Action> = {
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
  Enter: "select",
  Escape: "back",
  Backspace: "back",
  PageUp: "prevPlatform",
  PageDown: "nextPlatform",
  q: "prevPlatform",
  e: "nextPlatform",
  Home: "prevLetter",
  End: "nextLetter",
  f: "favorite",
  F1: "menu",
};

type Handler = (action: Action) => boolean;

// Topmost handler gets the action first (overlays sit above views).
const stack: Handler[] = [];

export function pushHandler(h: Handler): () => void {
  stack.push(h);
  return () => {
    const i = stack.lastIndexOf(h);
    if (i >= 0) stack.splice(i, 1);
  };
}

export function dispatch(action: Action): boolean {
  for (let i = stack.length - 1; i >= 0; i--) if (stack[i](action)) return true;
  return false;
}

function isTextField(t: EventTarget | null): boolean {
  if (!(t instanceof HTMLElement)) return false;
  return t.isContentEditable || t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT";
}

export function installKeyboard(): () => void {
  const onKey = (e: KeyboardEvent) => {
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    // In text fields only Escape is ours (to leave the field); the rest is typing.
    if (isTextField(e.target)) {
      if (e.key === "Escape") (e.target as HTMLElement).blur();
      return;
    }
    const action = KEYMAP[e.key];
    if (action && dispatch(action)) e.preventDefault();
  };
  window.addEventListener("keydown", onKey);
  return () => window.removeEventListener("keydown", onKey);
}
