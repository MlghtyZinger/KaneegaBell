// Row layout and D-pad movement for the game grid. Pure, unit-tested in
// tests/gridNav.test.ts.

export type NavDir = "up" | "down" | "left" | "right";

export interface Rows {
  /** First item of each row, and how many items it holds. */
  start: number[];
  len: number[];
  /** Row of each item. */
  row: number[];
  /** Group key for rows that begin a group (they get a header), else null. */
  header: (string | null)[];
}

/**
 * Splits `n` items into rows of up to `cols`. With `groupOf`, a new row (with
 * a header) starts whenever the group changes, so a row only ever holds one
 * platform's games and their covers share a shape.
 */
export function buildRows(n: number, cols: number, groupOf?: (i: number) => string | null): Rows {
  const rows: Rows = { start: [], len: [], row: new Array(n), header: [] };
  let prev: string | null | undefined = undefined;
  for (let i = 0; i < n; i++) {
    const key = groupOf ? groupOf(i) : null;
    const last = rows.len.length - 1;
    const newGroup = groupOf !== undefined && key !== prev;
    if (last < 0 || rows.len[last] >= cols || newGroup) {
      rows.start.push(i);
      rows.len.push(0);
      rows.header.push(newGroup ? key : null);
    }
    rows.len[rows.len.length - 1]++;
    rows.row[i] = rows.len.length - 1;
    prev = key;
  }
  return rows;
}

/**
 * Left/right walk the list (previous/next game, across rows and groups);
 * up/down move to the same position in the row above/below, or that row's
 * last game when it's shorter.
 */
export function gridMove(rows: Rows, i: number, dir: NavDir): number {
  const n = rows.row.length;
  if (n === 0 || i < 0 || i >= n) return i;
  if (dir === "left") return Math.max(0, i - 1);
  if (dir === "right") return Math.min(n - 1, i + 1);
  const r = rows.row[i];
  const target = r + (dir === "up" ? -1 : 1);
  if (target < 0 || target >= rows.start.length) return i;
  const pos = i - rows.start[r];
  return rows.start[target] + Math.min(pos, rows.len[target] - 1);
}
