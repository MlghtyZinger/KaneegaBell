import { createEffect, createMemo, createSignal, For, onCleanup, onMount, type JSX } from "solid-js";
import { stackLayout, type NavDir } from "./gridNav";

export type { NavDir };
/** The item to focus when moving from `index` in `dir` (may be `index` itself). */
export type Navigator = (index: number, dir: NavDir) => number;

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
   * Both read A–Z left to right, row after row. false: rows, each as tall as
   * its tallest card (covers aligned at the top). true: stacked, each card
   * right under the one above it in its column (see stackLayout).
   */
  stacked: boolean;
  gap: number;
  /** `item` is an accessor: a cell is reused when the list changes under it. */
  renderItem: (item: () => T, focused: () => boolean) => JSX.Element;
  /** Hands out the layout-aware neighbor lookup for D-pad navigation. */
  onNavigator?: (nav: Navigator) => void;
}

/** Keeps odd covers (banners, long boxes) from making absurd cards. */
const clampAspect = (a: number) => Math.min(1.6, Math.max(0.5, a || 1.4));

interface Layout {
  x: Float64Array;
  y: Float64Array;
  h: Float64Array;
  /** Row of each item (index / columns, in both layouts). */
  row: Int32Array;
  /** Item indices sorted by y, for finding what's on screen. */
  byY: Int32Array;
  maxH: number;
  total: number;
}

/**
 * Virtualized card grid: only cards inside the viewport (plus overscan) exist
 * in the DOM. Cards share one width and take their cover's height. Cards are
 * keyed by item index, so scrolling only creates/destroys the cards entering
 * or leaving view. Positioning uses transforms, so scrolling never triggers
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
  // mode changes, never while scrolling.
  const layout = createMemo<Layout>(() => {
    const items = props.items;
    const n = items.length;
    const c = cols();
    const w = cardW();
    const gap = props.gap;
    const x = new Float64Array(n);
    const y = new Float64Array(n);
    const h = new Float64Array(n);
    const row = new Int32Array(n);
    let maxH = 0;
    for (let i = 0; i < n; i++) {
      h[i] = w * clampAspect(props.itemAspect(items[i]));
      maxH = Math.max(maxH, h[i]);
      row[i] = Math.floor(i / c);
      x[i] = (i % c) * (w + gap);
    }
    let total = 0;
    if (props.stacked) {
      // Cards packed per column; columns re-line up once a card width apart.
      const p = stackLayout(h, c, gap, w);
      y.set(p.y);
      total = p.total + gap;
    } else {
      for (let r = 0; r * c < n; r++) {
        let rowH = 0;
        for (let i = r * c; i < Math.min(n, (r + 1) * c); i++) {
          y[i] = total;
          rowH = Math.max(rowH, h[i]);
        }
        total += rowH + gap;
      }
    }
    const byY = Int32Array.from({ length: n }, (_, i) => i).sort((a, b) => y[a] - y[b]);
    return { x, y, h, row, byY, maxH, total: Math.max(0, total - gap) };
  });

  const visible = createMemo(() => {
    const { y, h, byY, maxH } = layout();
    if (byY.length === 0 || cardW() <= 0) return [];
    const top = scrollTop() - padTop() - OVERSCAN_PX;
    const bottom = scrollTop() - padTop() + height() + OVERSCAN_PX;
    // First item that could reach into view: y >= top - maxH.
    let lo = 0;
    let hi = byY.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (y[byY[mid]] < top - maxH) lo = mid + 1;
      else hi = mid;
    }
    const out: number[] = [];
    for (let k = lo; k < byY.length && y[byY[k]] <= bottom; k++) {
      const i = byY[k];
      if (y[i] + h[i] >= top) out.push(i);
    }
    return out.sort((a, b) => a - b);
  });

  const navigate: Navigator = (i, dir) => {
    const L = layout();
    const n = L.y.length;
    if (n === 0 || i < 0 || i >= n) return i;
    const c = cols();
    // Both layouts read like text: left/right walk the list (the next game
    // A–Z), up/down move a row within the same column.
    if (dir === "left") return Math.max(0, i - 1);
    if (dir === "right") return Math.min(n - 1, i + 1);
    if (dir === "up") return i - c >= 0 ? i - c : i;
    return i + c < n ? i + c : L.row[i] < L.row[n - 1] ? n - 1 : i;
  };
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

  // Keep the focused card fully in view, with room for its focus ring and zoom.
  createEffect(() => {
    const i = props.focused;
    const { y, h } = layout();
    if (i < 0 || i >= y.length) return;
    const margin = props.gap;
    const cardTop = padTop() + y[i] - margin;
    const cardBottom = padTop() + y[i] + h[i] + margin;
    let target: number | null = null;
    if (cardTop < el.scrollTop) target = Math.max(0, cardTop);
    else if (cardBottom > el.scrollTop + height()) target = cardBottom - height();
    if (target !== null) {
      el.scrollTop = target;
      // Render the destination cards now rather than on the next scroll frame.
      setScrollTop(el.scrollTop);
    }
  });

  return (
    <div class="vgrid" ref={el} role="grid">
      <div class="vgrid-sizer" style={{ height: `${layout().total}px` }}>
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
