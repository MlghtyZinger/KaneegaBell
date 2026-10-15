// Which series to show for a game (details screen and sidebar).
import type { GameMetadata, Series } from "../api";
import { library } from "./library";

/** A series' games that are in the library list, in release order. */
export function owned(s: Series) {
  return s.games.map((id) => library.games().find((r) => r.id === id)).filter((r): r is NonNullable<typeof r> => !!r);
}

/**
 * The series to show: a real series first, else a franchise, and only one you
 * have at least two games from (one game alone says nothing worth showing).
 */
export function mainSeries(meta: GameMetadata): Series | null {
  const shown = meta.series.filter((s) => owned(s).length >= 2);
  return shown.find((s) => s.kind === "series") ?? shown[0] ?? null;
}
