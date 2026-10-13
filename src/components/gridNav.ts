// D-pad movement in the stacked (masonry) grid. Pure geometry, unit-tested in
// tests/gridNav.test.ts.

export type NavDir = "up" | "down" | "left" | "right";

export interface StackedLayout {
  /** Card tops and heights, by item index. */
  y: ArrayLike<number>;
  h: ArrayLike<number>;
  /** Column of each item, and each column's items top to bottom. */
  col: ArrayLike<number>;
  columns: number[][];
  gap: number;
}

export interface Move {
  index: number;
  /**
   * The horizontal line sideways moves follow. Passed back on the next
   * sideways move, it keeps a run of ← / → on one line (so → then ← returns
   * to the same card) instead of drifting up or down with each column's
   * card heights. Null after up/down.
   */
  line: number | null;
}

const center = (L: StackedLayout, i: number) => L.y[i] + L.h[i] / 2;

/** The card in `column` whose slot (card plus the gap below it) holds `line`, else the nearest. */
function cardAt(L: StackedLayout, column: number[], line: number): number {
  let best = column[0];
  let bestDist = Infinity;
  for (const j of column) {
    const top = L.y[j];
    const bottom = top + L.h[j] + L.gap;
    if (line >= top && line < bottom) return j;
    const d = line < top ? top - line : line - bottom;
    if (d < bestDist) {
      bestDist = d;
      best = j;
    }
  }
  return best;
}

/**
 * Up/down stay in the column. Left/right go to the card beside this one on
 * the current line; past the edge they wrap to the next line down (→) or up
 * (←), so holding → keeps moving down the grid to the end.
 */
export function stackedMove(L: StackedLayout, i: number, dir: NavDir, line: number | null): Move {
  const k = L.col[i];
  const column = L.columns[k];
  const p = column.indexOf(i);
  if (dir === "up") return { index: p > 0 ? column[p - 1] : i, line: null };
  if (dir === "down") return { index: p < column.length - 1 ? column[p + 1] : i, line: null };

  const at = line ?? center(L, i);
  const step = dir === "left" ? -1 : 1;
  const target = L.columns[k + step];
  if (target && target.length > 0) return { index: cardAt(L, target, at), line: at };

  // Wrap like reading text: → continues in the leftmost column that has a
  // card below the line, ← in the rightmost column with one above it.
  if (L.columns.length < 2) return { index: i, line: at };
  if (dir === "right") {
    for (const c of L.columns) {
      const j = c.find((x) => L.y[x] > at);
      if (j !== undefined) return { index: j, line: center(L, j) };
    }
  } else {
    for (let c = L.columns.length - 1; c >= 0; c--) {
      const above = L.columns[c].filter((x) => L.y[x] + L.h[x] + L.gap <= at);
      const j = above[above.length - 1];
      if (j !== undefined) return { index: j, line: center(L, j) };
    }
  }
  return { index: i, line: at };
}
