import type { JSX } from "solid-js";

/**
 * Small line icons drawn for KB (no icon font). The platform ones are
 * placeholders by kind (pad, handheld, disc) until milestone m's own icons.
 */
export type IconName =
  | "all"
  | "star"
  | "clock"
  | "stack"
  | "search"
  | "filter"
  | "tv"
  | "gear"
  | "pin"
  | "gamepad"
  | "handheld"
  | "disc"
  | "close"
  | "railHover"
  | "railClosed"
  | "pencil";

const PATHS: Record<IconName, () => JSX.Element> = {
  all: () => (
    <>
      <rect x="3.5" y="3.5" width="7" height="7" rx="1.6" />
      <rect x="13.5" y="3.5" width="7" height="7" rx="1.6" />
      <rect x="3.5" y="13.5" width="7" height="7" rx="1.6" />
      <rect x="13.5" y="13.5" width="7" height="7" rx="1.6" />
    </>
  ),
  star: () => <path d="M12 3.2l2.7 5.6 6.1.8-4.5 4.2 1.1 6.1L12 17l-5.4 2.9 1.1-6.1-4.5-4.2 6.1-.8z" />,
  clock: () => (
    <>
      <circle cx="12" cy="12" r="8.5" />
      <path d="M12 7.5V12l3 2" />
    </>
  ),
  stack: () => (
    <>
      <rect x="8" y="3" width="12" height="14" rx="2" opacity="0.55" />
      <rect x="4" y="7" width="12" height="14" rx="2" />
    </>
  ),
  search: () => (
    <>
      <circle cx="10.5" cy="10.5" r="6.5" />
      <path d="M15.5 15.5L20.5 20.5" />
    </>
  ),
  filter: () => <path d="M4 5h16l-6.2 7.6V19l-3.6 1.8v-8.2z" />,
  tv: () => (
    <>
      <rect x="3" y="4.5" width="18" height="12.5" rx="2" />
      <path d="M8.5 20.5h7M12 17v3.5" />
    </>
  ),
  gear: () => (
    <>
      <circle cx="12" cy="12" r="3" />
      <path d="M12 3v2.6M12 18.4V21M3 12h2.6M18.4 12H21M5.6 5.6l1.9 1.9M16.5 16.5l1.9 1.9M5.6 18.4l1.9-1.9M16.5 7.5l1.9-1.9" />
      <circle cx="12" cy="12" r="6.2" />
    </>
  ),
  pin: () => <path d="M9 3.5h6l-1 6.5 3.5 3.5h-11L10 10zM12 13.5V21" />,
  gamepad: () => (
    <>
      <path d="M7.2 7.5h9.6a4.2 4.2 0 0 1 4.2 4.2v1.6a3.6 3.6 0 0 1-6.5 2.2l-1.1-1.5h-2.8l-1.1 1.5A3.6 3.6 0 0 1 3 13.3v-1.6a4.2 4.2 0 0 1 4.2-4.2z" />
      <path d="M7.6 10.4v3.4M5.9 12.1h3.4" />
      <circle cx="16" cy="11" r="0.6" />
      <circle cx="17.6" cy="13" r="0.6" />
    </>
  ),
  handheld: () => (
    <>
      <rect x="6" y="2.5" width="12" height="19" rx="2.5" />
      <rect x="8.5" y="5" width="7" height="6.5" rx="1" />
      <path d="M9.6 15v3M8.1 16.5h3" />
      <circle cx="15" cy="16.5" r="0.8" />
    </>
  ),
  disc: () => (
    <>
      <circle cx="12" cy="12" r="8.8" />
      <circle cx="12" cy="12" r="2.4" />
      <path d="M12 6.2a5.8 5.8 0 0 1 5.8 5.8" opacity="0.6" />
    </>
  ),
  close: () => <path d="M6 6l12 12M18 6L6 18" />,
  pencil: () => <path d="M4 20l1-4.5L15.5 5a2.1 2.1 0 0 1 3 3L8 18.5zM13.5 7l3 3" />,
  // A sidebar that opens (arrow out) / stays folded (arrow in).
  railHover: () => (
    <>
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <path d="M9 4v16M13 12h5M15.8 9.6l2.4 2.4-2.4 2.4" />
    </>
  ),
  railClosed: () => (
    <>
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <path d="M9 4v16M18 12h-5M15.2 9.6L12.8 12l2.4 2.4" />
    </>
  ),
};

export function Icon(props: { name: IconName; class?: string }) {
  return (
    <svg
      class={`icon ${props.class ?? ""}`}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="1.8"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
    >
      {PATHS[props.name]()}
    </svg>
  );
}

const HANDHELD = new Set(["gba", "nds", "3ds", "switch", "psp"]);
const DISC = new Set(["gc", "wii", "segacd", "saturn", "ps1", "ps2", "ps3"]);
const FAMILY: Record<string, string> = {
  snes: "nintendo",
  n64: "nintendo",
  gba: "nintendo",
  gc: "nintendo",
  nds: "nintendo",
  wii: "nintendo",
  "3ds": "nintendo",
  switch: "nintendo",
  segacd: "sega",
  saturn: "sega",
  ps1: "sony",
  ps2: "sony",
  psp: "sony",
  ps3: "sony",
};

/** A platform's (placeholder) icon, tinted by maker family. */
export function PlatformIcon(props: { id: string; class?: string }) {
  const kind = (): IconName => (HANDHELD.has(props.id) ? "handheld" : DISC.has(props.id) ? "disc" : "gamepad");
  return <Icon name={kind()} class={`platform-icon fam-${FAMILY[props.id] ?? "other"} ${props.class ?? ""}`} />;
}
