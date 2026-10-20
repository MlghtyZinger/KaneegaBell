import { createSignal } from "solid-js";
import { api, type AppliedTheme } from "../api";

/** Set by the window's start-up script (src-tauri/src/themes.rs) before the page loads. */
type Loaded = AppliedTheme & { sheet: CSSStyleSheet };
const loaded = () => (window as unknown as { __kbTheme?: Loaded }).__kbTheme;

/** How each style marks the focused cover, in the picker's words. */
export const FOCUS_LOOKS: Record<string, string> = {
  glow: "Glow",
  ring: "Gap ring",
  lift: "Lift",
  brackets: "Brackets",
  bar: "Bar",
  spotlight: "Spotlight",
};

/** The bull in each style's colors, the same pictures as the window icon. */
const ICONS = Object.fromEntries(
  Object.entries(import.meta.glob<string>("../../src-tauri/icons/themes/*.png", { eager: true, import: "default" })).map(
    ([path, url]) => [path.split("/").pop()!.replace(".png", ""), url],
  ),
);
const [icon, setIcon] = createSignal("ember");
export const themeIcon = () => ICONS[icon()] ?? ICONS.ember;

/** Marks the page with the start-up style's focus look and icon (its colors are already in place). */
export type BorderShape = "default" | "sharp" | "rounded" | "bold";
export type BorderColor = "default" | "accent" | "gold" | "focus";
export const BORDER_SHAPES: Record<BorderShape, string> = { default: "Style's own", sharp: "Sharp", rounded: "Rounded", bold: "Bold" };
export const BORDER_COLORS: Record<BorderColor, string> = { default: "Style's own", accent: "Accent", gold: "Gold", focus: "Focus color" };

/** Overrides on top of any style: set on <html>, so swapping styles keeps them. */
export function applyBorders(shape: BorderShape, color: BorderColor) {
  const st = document.documentElement.style;
  const set = (k: string, v: string | null) => (v ? st.setProperty(k, v) : st.removeProperty(k));
  set("--radius", { sharp: "0px", rounded: "14px", bold: null, default: null }[shape]);
  set("--border-w", shape === "bold" ? "2px" : null);
  set("--border", color === "default" ? null : `var(--${color})`);
}

export function loadBorders() {
  void Promise.all([api.getSetting<BorderShape>("borderShape"), api.getSetting<BorderColor>("borderColor")])
    .then(([s, c]) => applyBorders(s ?? "default", c ?? "default"))
    .catch(() => {});
}

export function initTheme() {
  loadBorders();
  document.documentElement.dataset.focusLook = loaded()?.focusLook ?? "brackets";
  setIcon(loaded()?.icon ?? "ember");
}

/** Swaps in another style without reloading the page. */
export function applyTheme(t: AppliedTheme) {
  let sheet = loaded()?.sheet;
  if (!sheet) {
    sheet = new CSSStyleSheet();
    document.adoptedStyleSheets = [...document.adoptedStyleSheets, sheet];
  }
  sheet.replaceSync(t.css);
  (window as unknown as { __kbTheme: Loaded }).__kbTheme = { ...t, sheet };
  document.documentElement.dataset.focusLook = t.focusLook;
  setIcon(t.icon);
}
