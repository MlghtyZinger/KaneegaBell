export const MAKERS = [
  { id: "nintendo", name: "Nintendo", mark: "N" },
  { id: "sony", name: "PlayStation", mark: "PS" },
  { id: "sega", name: "Sega", mark: "S" },
  { id: "microsoft", name: "Microsoft", mark: "X" },
  { id: "other", name: "Other", mark: "…" },
] as const;
export type Maker = (typeof MAKERS)[number]["id"];

const FAMILY: Record<string, Maker> = {
  snes: "nintendo", n64: "nintendo", gba: "nintendo", gc: "nintendo", nds: "nintendo",
  wii: "nintendo", "3ds": "nintendo", switch: "nintendo",
  segacd: "sega", saturn: "sega",
  ps1: "sony", ps2: "sony", psp: "sony", ps3: "sony",
  xbox: "microsoft", xbox360: "microsoft",
};
export const platformMaker = (id: string): Maker => FAMILY[id] ?? "other";

export type PlatformRailRow =
  | { kind: "tab"; id: string; child: boolean }
  | { kind: "maker"; id: string; maker: (typeof MAKERS)[number]; platforms: string[] };

/** Folding affects sidebar rows only; the library's platform-cycle list stays flat. */
export function platformRailRows(tabs: readonly string[], platforms: readonly string[], open: readonly string[]): PlatformRailRow[] {
  const ids = new Set(platforms);
  const rows: PlatformRailRow[] = tabs.filter((id) => !ids.has(id)).map((id) => ({ kind: "tab", id, child: false }));
  for (const maker of MAKERS) {
    const children = tabs.filter((id) => ids.has(id) && platformMaker(id) === maker.id);
    if (!children.length) continue;
    rows.push({ kind: "maker", id: `maker:${maker.id}`, maker, platforms: children });
    if (open.includes(maker.id)) rows.push(...children.map((id) => ({ kind: "tab" as const, id, child: true })));
  }
  return rows;
}
