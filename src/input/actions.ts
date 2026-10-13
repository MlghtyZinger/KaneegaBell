// Abstract UI actions. The keyboard maps here, and the Rust gamepad thread
// emits the same actions, so views never care which device is used.
import { createRoot, createSignal } from "solid-js";

export type Action =
  | "up"
  | "down"
  | "left"
  | "right"
  | "select"
  | "back"
  | "secondary"
  | "prevPlatform"
  | "nextPlatform"
  | "prevLetter"
  | "nextLetter"
  | "favorite"
  | "menu"
  | "zoomIn"
  | "zoomOut";

const KEYMAP: Record<string, Action> = {
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
  Enter: "select",
  Escape: "back",
  Backspace: "back",
  "/": "secondary",
  PageUp: "prevPlatform",
  PageDown: "nextPlatform",
  q: "prevPlatform",
  e: "nextPlatform",
  Home: "prevLetter",
  End: "nextLetter",
  f: "favorite",
  F1: "menu",
  "+": "zoomIn",
  "=": "zoomIn",
  "-": "zoomOut",
};

export type Device = "pointer" | "keyboard" | "gamepad";

/** The device used last: drives focus-ring visibility and button-hint glyphs. */
export const { device, setDevice } = createRoot(() => {
  const [device, setDeviceRaw] = createSignal<Device>("pointer");
  const setDevice = (d: Device) => {
    if (device() === d) return;
    setDeviceRaw(d);
    // Focus rings only when navigating without a mouse.
    document.documentElement.classList.toggle("nav-mode", d !== "pointer");
  };
  return { device, setDevice };
});

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

const TEXT_INPUT_TYPES = new Set(["text", "search", "number", "password", "email", "url", "tel", ""]);

/** Fields where keys are typing (checkboxes and buttons are not). */
export function isTextField(t: EventTarget | null): t is HTMLInputElement | HTMLTextAreaElement {
  if (!(t instanceof HTMLElement)) return false;
  if (t.isContentEditable || t.tagName === "TEXTAREA") return true;
  return t instanceof HTMLInputElement && TEXT_INPUT_TYPES.has(t.type);
}

/** Lets an overlay (the on-screen keyboard) see raw key presses first. */
let keyInterceptor: ((e: KeyboardEvent) => boolean) | null = null;
export function setKeyInterceptor(f: ((e: KeyboardEvent) => boolean) | null) {
  keyInterceptor = f;
}

export function installKeyboard(): () => void {
  const onKey = (e: KeyboardEvent) => {
    // Ctrl +/- zoom the card grid (like a browser zooms a page).
    if (e.ctrlKey && !e.altKey && !e.metaKey && !isTextField(e.target)) {
      const zoom = ({ "+": "zoomIn", "=": "zoomIn", "-": "zoomOut" } as Record<string, Action>)[e.key];
      if (zoom && dispatch(zoom)) e.preventDefault();
      return;
    }
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    setDevice("keyboard");
    if (keyInterceptor?.(e)) {
      e.preventDefault();
      return;
    }
    // In text fields only Escape is ours (to leave the field); the rest is typing.
    if (isTextField(e.target)) {
      if (e.key === "Escape") (e.target as HTMLElement).blur();
      return;
    }
    const action = KEYMAP[e.key];
    if (action && dispatch(action)) e.preventDefault();
  };
  const onPointer = () => setDevice("pointer");
  window.addEventListener("keydown", onKey);
  window.addEventListener("mousedown", onPointer, { passive: true });
  window.addEventListener("mousemove", onPointer, { passive: true });
  return () => {
    window.removeEventListener("keydown", onKey);
    window.removeEventListener("mousedown", onPointer);
    window.removeEventListener("mousemove", onPointer);
  };
}

/** Entry point for controller actions from Rust. */
export function dispatchFromGamepad(action: Action) {
  setDevice("gamepad");
  dispatch(action);
}
