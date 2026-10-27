import type { CoverCandidate } from "../api";

/** Every picture the artwork studio changes, and the game's manual. */
export type Slot = "cover" | "hero" | "logo" | "icon" | "back" | "spine" | "disc" | "manual";

/** One staged change for a slot: a candidate, an image file, removal, or the cover as the icon. */
export type Pick =
  | { op: "option"; c: CoverCandidate }
  | { op: "file"; path: string }
  | { op: "remove" }
  | { op: "useCover" };

/** The pictures on a game's 3D case (LaunchBox's, or an image file of your own). */
export const CASE_SLOTS: readonly Slot[] = ["back", "spine", "disc"];
/** Slots that accept an image file of your own (backgrounds and logos come from the services only). */
export const FILE_SLOTS: readonly Slot[] = ["cover", "icon", "back", "spine", "disc", "manual"];
/** Slots that can be removed. */
export const REMOVABLE: readonly Slot[] = ["cover", "hero", "logo", "icon", "manual"];

/** What applying one staged pick does, as a plain description (see `ArtworkStudio` for the calls). */
export type ApplyCall =
  | { op: "set"; source: CoverCandidate["source"] | "file"; name: string; url: string }
  | { op: "removeCover" }
  | { op: "removeExtra"; kind: "hero" | "logo" | "icon" }
  | { op: "useCover" }
  | { op: "manualPick"; url: string; label: string }
  | { op: "manualFile"; path: string }
  | { op: "manualRemove" };

export function applyCall(slot: Slot, pick: Pick): ApplyCall {
  if (slot === "manual") {
    if (pick.op === "option") return { op: "manualPick", url: pick.c.url, label: pick.c.label };
    if (pick.op === "file") return { op: "manualFile", path: pick.path };
    return { op: "manualRemove" };
  }
  switch (pick.op) {
    case "option":
      return { op: "set", source: pick.c.source, name: pick.c.name, url: pick.c.url };
    case "file":
      return { op: "set", source: "file", name: pick.path, url: pick.path };
    case "remove":
      return slot === "cover" ? { op: "removeCover" } : { op: "removeExtra", kind: slot as "hero" | "logo" | "icon" };
    case "useCover":
      return { op: "useCover" };
  }
}
