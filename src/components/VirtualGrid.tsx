import { createEffect, createMemo, createSignal, For, onCleanup, onMount, type JSX } from "solid-js";
import { buildRows, gridMove, type NavDir, type Rows } from "./gridNav";

export type { NavDir };
/**
 * The item to focus when moving from `index` in `dir` (may be `index` itself);
 * `atRowStart` tells whether an item is the first of its row.
 */
export type Navigator = ((index: number, dir: NavDir) => number) & { atRowStart: (index: number) => boolean };

interface Props<T> {
  items: T[];
  /** Index of the focused item, or -1. */
  focused: number;
  onFocus: (index: number) => void;
  onActivate: (index: number) => void;
  /** Smallest card width in px; columns are derived from the container width. */
  minCardWidth: number;
  /** Card height / width per item (its cover's shape). */
  itemAspect: (item: T) => number;
  /**
   * Group items (platforms in mixed views): each group starts a new row under
   * a header, so a row holds one group's items. Omit for plain rows.
   */
  groupOf?: (item: T) => string;
  groupLabel?: (key: string) => string;
  gap: number;
  /** `item` is an accessor: a cell is reused when the list changes under it. */
  renderItem: (item: () => T, focused: () => boolean) => JSX.Element;
  /** Hands out the layout-aware neighbor lookup for D-pad navigation. */
  onNavigator?: (nav: Navigator) => void;
}

/** Keeps odd covers (banners, long boxes) from making absurd cards. */
const clampAspect = (a: number) => Math.min(1.6, Math.max(0.5, a || 1.4));
/** Space a group header takes above its first row (label band + breathing room). */
const HEADER_H = 58;
const HEADER_LABEL_H = 44;

interface Layout {
  rows: Rows;
  x: Float64Array;
  y: Float64Array;
  h: Float64Array;
  /** Top of each row (its header above it when it has one). */
  rowTop: Float64Array;
  headers: { key: string; y: number }[];
  total: number;
}

/**
 * Virtualized card grid: only rows inside the viewport (plus overscan) exist
 * in the DOM. Cards share one width and take their cover's height; each row
 * is as tall as its tallest card (covers aligned at the top). Cards are keyed
 * by item index, so scrolling only creates/destroys the rows entering or
 * leaving view. Positioning uses transforms, so scrolling never triggers
 * layout.
 */
export function VirtualGrid<T>(props: Props<T>) {
  let el!: HTMLDivElement;
  const [width, setWidth] = createSignal(0);
  const [height, setHeight] = createSignal(0);
  const [scrollTop, setScrollTop] = createSignal(0);
  // The container's top padding: card y positions are offset by it in scroll space.
  const [padTop, setPadTop] = createSignal(0);

  const OVERSCAN_PX = 600;

  const cols = createMemo(() => Math.max(1, Math.floor((width() + props.gap) / (props.minCardWidth + props.gap))));
  const cardW = createMemo(() => (width() - props.gap * (cols() - 1)) / cols());

  // Positions: recomputed when the list, the columns, the card width or the
  // grouping changes, never while scrolling.
  const layout = createMemo<Layout>(() => {
    const items = props.items;
    const n = items.length;
    const w = cardW();
    const gap = props.gap;
    const groupOf = props.groupOf;
    const rows = buildRows(n, cols(), groupOf ? (i) => groupOf(items[i]) : undefined);
    const x = new Float64Array(n);
    const y = new Float64Array(n);
    const h = new Float64Array(n);
    const rowTop = new Float64Array(rows.start.length);
    const headers: { key: string; y: number }[] = [];
    let top = 0;
    for (let r = 0; r < rows.start.length; r++) {
      const key = rows.header[r];
      if (key !== null) {
        // A little extra space before every group but the first.
        if (r > 0) top += gap;
        headers.push({ key, y: top });
        top += HEADER_H;
      }
      rowTop[r] = key !== null ? top - HEADER_H : top;
      let rowH = 0;
      for (let k = 0; k < rows.len[r]; k++) {
        const i = rows.start[r] + k;
        h[i] = w * clampAspect(props.itemAspect(items[i]));
        x[i] = k * (w + gap);
        y[i] = top;
        rowH = Math.max(rowH, h[i]);
      }
      top += rowH + gap;
    }
    return { rows, x, y, h, rowTop, headers, total: Math.max(0, top - gap) };
  });

  const visible = createMemo(() => {
    const { rows, rowTop } = layout();
    const count = rows.start.length;
    if (count === 0 || cardW() <= 0) return [];
    const top = scrollTop() - padTop() - OVERSCAN_PX;
    const bottom = scrollTop() - padTop() + height() + OVERSCAN_PX;
    // First row whose next row starts below the top edge.
    let lo = 0;
    let hi = count - 1;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      const nextTop = mid + 1 < count ? rowTop[mid + 1] : Infinity;
      if (nextTop <= top) lo = mid + 1;
      else hi = mid;
    }
    const out: number[] = [];
    for (let r = lo; r < count && rowTop[r] <= bottom; r++) {
      for (let k = 0; k < rows.len[r]; k++) out.push(rows.start[r] + k);
    }
    return out;
  });

  const navigate: Navigator = Object.assign((i: number, dir: NavDir) => gridMove(layout().rows, i, dir), {
    atRowStart: (i: number) => {
      const r = layout().rows;
      return i >= 0 && i < r.row.length && r.start[r.row[i]] === i;
    },
  });
  onMount(() => props.onNavigator?.(navigate));

  onMount(() => {
    // Measure synchronously: ResizeObserver's first callback waits for a
    // rendering pass, which never happens while the window is still hidden.
    const measure = () => {
      const cs = getComputedStyle(el);
      setWidth(el.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight));
      setHeight(el.clientHeight);
      setPadTop(parseFloat(cs.paddingTop));
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    let raf = 0;
    const onScroll = () => {
      if (raf) return;
      raf = requestAnimationFrame(() => {
        raf = 0;
        setScrollTop(el.scrollTop);
      });
    };
    el.addEventListener("scroll", onScroll, { passive: true });
    onCleanup(() => {
      ro.disconnect();
      el.removeEventListener("scroll", onScroll);
      cancelAnimationFrame(raf);
    });
  });

  // Keep the focused card fully in view (with its group header, on a group's
  // first row), with room for its focus ring and zoom.
  createEffect(() => {
    const i = props.focused;
    const { rows, y, h, rowTop } = layout();
    if (i < 0 || i >= y.length) return;
    const margin = props.gap;
    const cardTop = padTop() + rowTop[rows.row[i]] - margin;
    const cardBottom = padTop() + y[i] + h[i] + margin;
    let target: number | null = null;
    if (cardTop < el.scrollTop) target = Math.max(0, cardTop);
    else if (cardBottom > el.scrollTop + height()) target = cardBottom - height();
    if (target !== null) {
      el.scrollTop = target;
      // Render the destination rows now rather than on the next scroll frame.
      setScrollTop(el.scrollTop);
    }
  });

  return (
    <div class="vgrid" ref={el} role="grid">
      <div class="vgrid-sizer" style={{ height: `${layout().total}px` }}>
        <For each={layout().headers}>
          {(hd) => (
            <div class="vgrid-header" style={{ transform: `translate3d(0, ${hd.y}px, 0)`, height: `${HEADER_LABEL_H}px` }}>
              {props.groupLabel?.(hd.key) ?? hd.key}
            </div>
          )}
        </For>
        <For each={visible()}>
          {(i) => {
            const isFocused = () => props.focused === i;
            return (
              <div
                class="vgrid-cell"
                role="gridcell"
                style={{
                  width: `${cardW()}px`,
                  height: `${layout().h[i] ?? 0}px`,
                  transform: `translate3d(${layout().x[i] ?? 0}px, ${layout().y[i] ?? 0}px, 0)`,
                }}
                onMouseDown={() => props.onFocus(i)}
                onDblClick={() => props.onActivate(i)}
              >
                {props.renderItem(() => props.items[i], isFocused)}
              </div>
            );
          }}
        </For>
      </div>
    </div>
  );
}
