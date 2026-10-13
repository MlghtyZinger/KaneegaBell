// Card placement for the stacked grid. Pure, unit-tested in tests/gridNav.test.ts.

export type NavDir = "up" | "down" | "left" | "right";

export interface Placement {
  /** Column of each item. */
  col: number[];
  /** Top of each item. */
  y: number[];
  /** Total height (without the trailing gap). */
  total: number;
}

/**
 * Stacked layout that reads like text: game i goes in column i % cols (so
 * A–Z runs left to right, row after row, exactly like the Rows layout, and the
 * D-pad works the same way), and each card sits right under the one above it
 * in its column, so different cover shapes don't leave gaps. Columns drift
 * apart as shapes vary; once they're more than `resync` apart at the start of
 * a row, they're lined up again (a small gap under the short columns), so a
 * card's neighbours stay beside it rather than far above or below.
 */
export function stackLayout(heights: ArrayLike<number>, cols: number, gap: number, resync: number): Placement {
  const bottom = new Array<number>(cols).fill(0);
  const col: number[] = [];
  const y: number[] = [];
  for (let i = 0; i < heights.length; i++) {
    const k = i % cols;
    if (k === 0 && i > 0) {
      const max = Math.max(...bottom);
      if (max - Math.min(...bottom) > resync) bottom.fill(max);
    }
    col.push(k);
    y.push(bottom[k]);
    bottom[k] += heights[i] + gap;
  }
  const used = Math.min(cols, heights.length);
  return { col, y, total: used ? Math.max(0, Math.max(...bottom.slice(0, used)) - gap) : 0 };
}
