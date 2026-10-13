import { createEffect, createMemo, createSignal, For, onCleanup, onMount, type JSX } from "solid-js";

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
  gap: number;
  /** `item` is an accessor: a cell is reused when the list changes under it. */
  renderItem: (item: () => T, focused: () => boolean) => JSX.Element;
  /** Reports the column count so arrow-key navigation can move by rows. */
  onColumns?: (cols: number) => void;
}

/** Keeps odd covers (banners, long boxes) from making absurd cards. */
const clampAspect = (a: number) => Math.min(1.6, Math.max(0.5, a || 1.4));

/**
 * Virtualized grid: only rows inside the viewport (plus a small overscan) exist
 * in the DOM. Cards share one width; each card's height follows its cover, and
 * each row is as tall as its tallest card (covers line up at the top), so a
 * row of wide SNES boxes is short and a row of PS2 cases tall.
 * Cards are keyed by item index, so scrolling only creates/destroys the rows
 * entering/leaving view. Positioning uses transforms, so scrolling never
 * triggers layout.
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

  createEffect(() => props.onColumns?.(cols()));

  // Per-item heights and per-row offsets: recomputed when the list, the
  // column count or the card width changes, never while scrolling.
  const layout = createMemo(() => {
    const items = props.items;
    const c = cols();
    const w = cardW();
    const heights = new Float64Array(items.length);
    for (let i = 0; i < items.length; i++) heights[i] = w * clampAspect(props.itemAspect(items[i]));
    const rows = Math.ceil(items.length / c);
    const top = new Float64Array(rows + 1); // top[r] = y of row r; top[rows] = total incl. trailing gap
    for (let r = 0; r < rows; r++) {
      let h = 0;
      for (let i = r * c; i < Math.min(items.length, (r + 1) * c); i++) h = Math.max(h, heights[i]);
      top[r + 1] = top[r] + h + props.gap;
    }
    return { heights, top, rows };
  });

  /** First row whose bottom is below `y`. */
  const rowAt = (y: number) => {
    const { top, rows } = layout();
    let lo = 0;
    let hi = rows;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (top[mid + 1] <= y) lo = mid + 1;
      else hi = mid;
    }
    return Math.min(lo, Math.max(0, rows - 1));
  };

  const visible = createMemo(() => {
    const { rows } = layout();
    if (rows === 0 || cardW() <= 0) return [];
    const y = scrollTop() - padTop();
    const first = rowAt(Math.max(0, y - OVERSCAN_PX));
    const last = rowAt(y + height() + OVERSCAN_PX);
    const out: number[] = [];
    const n = props.items.length;
    const c = cols();
    for (let r = first; r <= last; r++) {
      for (let k = 0; k < c; k++) {
        const i = r * c + k;
        if (i < n) out.push(i);
      }
    }
    return out;
  });

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
    const { top, heights, rows } = layout();
    if (i < 0 || i >= heights.length || rows === 0) return;
    const r = Math.floor(i / cols());
    const margin = props.gap;
    const cardTop = padTop() + top[r] - margin;
    const cardBottom = padTop() + top[r] + heights[i] + margin;
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
      <div class="vgrid-sizer" style={{ height: `${Math.max(0, layout().top[layout().rows] - props.gap)}px` }}>
        <For each={visible()}>
          {(i) => {
            const x = () => (i % cols()) * (cardW() + props.gap);
            const y = () => layout().top[Math.floor(i / cols())] ?? 0;
            const isFocused = () => props.focused === i;
            return (
              <div
                class="vgrid-cell"
                role="gridcell"
                style={{
                  width: `${cardW()}px`,
                  height: `${layout().heights[i] ?? 0}px`,
                  transform: `translate3d(${x()}px, ${y()}px, 0)`,
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
