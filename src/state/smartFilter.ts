import type { CollectionRules, GameRow } from "../api";

/** The empty filter: matches every game. */
export function emptyRules(): CollectionRules {
  return {};
}

const norm = (s?: string | null): string => (s ?? "").trim().toLowerCase();

/** Whether a game matches every active rule (all ANDed). */
export function matchesRules(g: GameRow, r: CollectionRules): boolean {
  if (r.platforms && r.platforms.length > 0 && !r.platforms.includes(g.platformId)) return false;
  if (r.genres && r.genres.length > 0 && !(g.genres ?? []).some((id) => r.genres!.includes(id))) return false;
  if (r.developers && r.developers.length > 0 && !r.developers.some((d) => norm(d) === norm(g.developer))) return false;
  if (r.publishers && r.publishers.length > 0 && !r.publishers.some((p) => norm(p) === norm(g.publisher))) return false;
  if (r.yearFrom != null && (g.year == null || g.year < r.yearFrom)) return false;
  if (r.yearTo != null && (g.year == null || g.year > r.yearTo)) return false;
  if (r.played === "played" && (g.playSeconds ?? 0) <= 0) return false;
  if (r.played === "unplayed" && (g.playSeconds ?? 0) > 0) return false;
  if (r.favorite && !g.favorite) return false;
  if (r.playFrom != null && (g.playSeconds ?? 0) < r.playFrom) return false;
  if (r.playTo != null && (g.playSeconds ?? 0) > r.playTo) return false;
  return true;
}

/** How many fields are set (for the "Filter (n)" label). */
export function countActive(r: CollectionRules): number {
  let n = 0;
  if (r.platforms?.length) n++;
  if (r.genres?.length) n++;
  if (r.developers?.length) n++;
  if (r.publishers?.length) n++;
  if (r.yearFrom != null || r.yearTo != null) n++;
  if (r.played) n++;
  if (r.favorite) n++;
  if (r.playFrom != null || r.playTo != null) n++;
  return n;
}

/** Distinct, sorted values for a picker list (case-insensitive). */
export function distinct(values: (string | null | undefined)[]): string[] {
  const seen = new Map<string, string>();
  for (const v of values) {
    const s = (v ?? "").trim();
    if (!s) continue;
    const k = s.toLowerCase();
    if (!seen.has(k)) seen.set(k, s);
  }
  return [...seen.values()].sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase()));
}
