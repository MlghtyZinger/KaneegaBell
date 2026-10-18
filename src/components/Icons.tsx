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

/** One drawn glyph per platform (24x24, stroke from `.icon`); unknown platforms fall back to a pad. */
const PLATFORM_GLYPHS: Record<string, () => JSX.Element> = {
  // Pad with the diamond of buttons.
  snes: () => (
    <>
      <rect x="2.5" y="7.5" width="19" height="9" rx="4.5" />
      <path d="M7 10.2v3.6M5.2 12h3.6" />
      <circle cx="15" cy="12" r="0.7" />
      <circle cx="18.2" cy="12" r="0.7" />
      <circle cx="16.6" cy="10.2" r="0.7" />
      <circle cx="16.6" cy="13.8" r="0.7" />
    </>
  ),
  // The three-handled pad.
  n64: () => (
    <>
      <path d="M8.6 8.5H7C5 8.5 3.4 12.2 3.4 17c0 1.6 2 1.8 2.8.4L9 12.5h6l2.8 4.9c.8 1.4 2.8 1.2 2.8-.4 0-4.8-1.6-8.5-3.6-8.5h-1.6M8.6 8.5h6.8M12 12.5V20" />
      <circle cx="12" cy="6" r="1.2" />
    </>
  ),
  gba: () => (
    <>
      <rect x="2.5" y="6.5" width="19" height="11" rx="3.5" />
      <rect x="9" y="9" width="6" height="6" rx="0.8" />
      <path d="M5.6 10.8v3M4.1 12.3h3" />
      <circle cx="18" cy="11.2" r="0.7" />
      <circle cx="17" cy="13.4" r="0.7" />
    </>
  ),
  // Cube with its carry handle and the round lid.
  gc: () => (
    <>
      <path d="M9 4.5V3h6v1.5" />
      <rect x="3.5" y="4.5" width="17" height="16" rx="3" />
      <circle cx="12" cy="12.5" r="4" />
      <path d="M12 8.5v4l2.8 2" opacity="0.7" />
    </>
  ),
  // Clamshell with two screens.
  nds: () => (
    <>
      <rect x="5" y="2.5" width="14" height="8.5" rx="2" />
      <rect x="5" y="13" width="14" height="8.5" rx="2" />
      <path d="M8 17.2h2.4M9.2 16v2.4M15 16.4h1M14 18h1" />
    </>
  ),
  "3ds": () => (
    <>
      <rect x="3.5" y="2.5" width="17" height="8.5" rx="2" />
      <rect x="5.5" y="13" width="13" height="8.5" rx="2" />
      <path d="M17.5 5v3.5" />
      <path d="M8.6 17.2H11M9.8 16v2.4" />
    </>
  ),
  // The slim remote.
  wii: () => (
    <>
      <rect x="8" y="2.5" width="8" height="19" rx="2.5" />
      <circle cx="12" cy="7.2" r="1.6" />
      <path d="M10.4 12.5h3.2M12 14.5v0" />
      <circle cx="12" cy="17.4" r="0.8" />
    </>
  ),
  // Screen between the two Joy-Con rails.
  switch: () => (
    <>
      <rect x="7" y="4" width="10" height="16" rx="1" />
      <path d="M7 4H5a2.5 2.5 0 0 0-2.5 2.5v11A2.5 2.5 0 0 0 5 20h2M17 4h2a2.5 2.5 0 0 1 2.5 2.5v11A2.5 2.5 0 0 1 19 20h-2" />
      <circle cx="4.9" cy="9" r="0.9" />
      <circle cx="19.1" cy="15" r="0.9" />
    </>
  ),
  // Disc on a base.
  segacd: () => (
    <>
      <circle cx="12" cy="9.5" r="6.5" />
      <circle cx="12" cy="9.5" r="1.8" />
      <path d="M2.5 20.5h19M5 18h14" />
    </>
  ),
  // The planet.
  saturn: () => (
    <>
      <circle cx="12" cy="12" r="5" />
      <ellipse cx="12" cy="12" rx="10" ry="3.4" transform="rotate(-22 12 12)" />
    </>
  ),
  // The four face symbols.
  ps1: () => (
    <>
      <path d="M12 3.2l2.2 3.8H9.8z" />
      <circle cx="19" cy="12" r="2.1" />
      <path d="M3.8 9.9l4.4 4.4M8.2 9.9l-4.4 4.4" />
      <rect x="9.9" y="16.4" width="4.2" height="4.2" />
    </>
  ),
  // Upright slab with vents.
  ps2: () => (
    <>
      <rect x="7.5" y="2.5" width="9" height="19" rx="1.4" />
      <path d="M10 6v12M12 6v12M14 6v12" opacity="0.6" />
    </>
  ),
  // Flat slab with the glossy curve.
  ps3: () => (
    <>
      <path d="M2.8 17V9.4a2 2 0 0 1 2-2h14.4a2 2 0 0 1 2 2V17a1.6 1.6 0 0 1-1.6 1.6H4.4A1.6 1.6 0 0 1 2.8 17z" />
      <path d="M2.8 11.2c6 1.4 12.4 1.4 18.4 0" opacity="0.6" />
      <circle cx="17.8" cy="15" r="0.7" />
    </>
  ),
  psp: () => (
    <>
      <rect x="1.8" y="7" width="20.4" height="10" rx="3.2" />
      <rect x="7" y="9" width="10" height="6" rx="0.8" />
      <path d="M4.2 10.8v2.8M2.9 12.2h2.6" />
      <circle cx="19.5" cy="11" r="0.6" />
      <circle cx="19.5" cy="13.4" r="0.6" />
    </>
  ),
  // The sphere with its X.
  xbox: () => (
    <>
      <circle cx="12" cy="12" r="9" />
      <path d="M7.2 7.4c2 1.4 3.2 3 4.8 4.6 1.6-1.6 2.8-3.2 4.8-4.6M7.2 16.6c2-1.4 3.2-3 4.8-4.6 1.6 1.6 2.8 3.2 4.8 4.6" />
    </>
  ),
  // The ring with its orb.
  xbox360: () => (
    <>
      <path d="M19.8 13.5A8.5 8.5 0 1 1 10.5 3.6" />
      <circle cx="16.8" cy="7.2" r="2.4" />
    </>
  ),
};

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
  xbox: "microsoft",
  xbox360: "microsoft",
};

/** A platform's icon, tinted by maker family. */
export function PlatformIcon(props: { id: string; class?: string }) {
  return (
    <svg
      class={`icon platform-icon fam-${FAMILY[props.id] ?? "other"} ${props.class ?? ""}`}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="1.7"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
    >
      {(PLATFORM_GLYPHS[props.id] ?? PATHS.gamepad)()}
    </svg>
  );
}

const MONOGRAMS: Record<string, string> = {
  retroarch: "RA",
  dolphin: "Do",
  duckstation: "DS",
  pcsx2: "P2",
  rpcs3: "R3",
  azahar: "Az",
  eden: "Ed",
  ryubing: "Ry",
  ryujinx: "Ry",
  citron: "Ci",
  sudachi: "Su",
  yuzu: "Yu",
  xemu: "Xe",
  xenia: "Xn",
};

/** Emulator badge: a rounded square with its monogram (own emulators use their first letters). */
export function EmulatorBadge(props: { id: string; name?: string }) {
  const text = () => MONOGRAMS[props.id] ?? (props.name ?? props.id).replace(/[^A-Za-z0-9]/g, "").slice(0, 2);
  return <span class="emu-badge" aria-hidden="true">{text()}</span>;
}
