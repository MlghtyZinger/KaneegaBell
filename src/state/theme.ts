import { createSignal } from "solid-js";
import type { AppliedTheme } from "../api";

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
  halo: "Halo",
  wii: "Channel edge",
};

/** The bull in each style's colors, the same pictures as the window icon. */
const ICONS = Object.fromEntries(
  Object.entries(import.meta.glob<string>("../../src-tauri/icons/themes/*.png", { eager: true, import: "default" })).map(
    ([path, url]) => [path.split("/").pop()!.replace(".png", ""), url],
  ),
);
const [icon, setIcon] = createSignal("forge");
export const [desktopStyle, setDesktopStyle] = createSignal("forge");
export const themeIcon = (id = icon()) => ICONS[id] ?? ICONS.forge;

/** Marks the page with the start-up style's focus look and icon (its colors are already in place). */
export function initTheme() {
  document.documentElement.dataset.focusLook = loaded()?.focusLook ?? "brackets";
  setIcon(loaded()?.icon ?? "forge");
  setDesktopStyle(loaded()?.id ?? "forge");
  document.documentElement.dataset.desktopStyle = desktopStyle();
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
  document.documentElement.dataset.desktopStyle = t.id;
  setDesktopStyle(t.id);
  setIcon(t.icon);
}
