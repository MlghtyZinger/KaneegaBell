import { createEffect, createMemo, createSignal, For, onCleanup, onMount, type JSX } from "solid-js";

interface Props<T> {
  items: T[];
  /** Index of the focused item, or -1. */
  focused: number;
  onFocus: (index: number) => void;
  onActivate: (index: number) => void;
  /** Smallest card width in px; columns are derived from the container width. */
  minCardWidth: number;
  /** Card height / width. */
  aspect: number;
  gap: number;
  /** `item` is an accessor: a cell is reused when the list changes under it. */
  renderItem: (item: () => T, focused: () => boolean) => JSX.Element;
  /** Reports the column count so arrow-key navigation can move by rows. */
  onColumns?: (cols: number) => void;
}

/**
 * Virtualized grid: only rows inside the viewport (plus a small overscan) exist
 * in the DOM. Cards are keyed by item index, so scrolling only creates/destroys
 * the rows entering/leaving view and never re-renders the ones that stay.
 * Positioning uses transforms, so scrolling never triggers layout.
 */
export function VirtualGrid<T>(props: Props<T>) {
  let el!: HTMLDivElement;
  const [width, setWidth] = createSignal(0);
  const [height, setHeight] = createSignal(0);
  const [scrollTop, setScrollTop] = createSignal(0);
  // The container's top padding: card y positions are offset by it in scroll space.
  const [padTop, setPadTop] = createSignal(0);

  const OVERSCAN_ROWS = 2;

  const cols = createMemo(() => Math.max(1, Math.floor((width() + props.gap) / (props.minCardWidth + props.gap))));
  const cardW = createMemo(() => (width() - props.gap * (cols() - 1)) / cols());
  const cardH = createMemo(() => cardW() * props.aspect);
  const rowH = createMemo(() => cardH() + props.gap);
  const rows = createMemo(() => Math.ceil(props.items.length / cols()));

  createEffect(() => props.onColumns?.(cols()));

  const visible = createMemo(() => {
    const rh = rowH();
    if (rh <= 0) return [];
    const y = scrollTop() - padTop();
    const first = Math.max(0, Math.floor(y / rh) - OVERSCAN_ROWS);
    const last = Math.min(rows() - 1, Math.ceil((y + height()) / rh) + OVERSCAN_ROWS);
    const out: number[] = [];
    const n = props.items.length;
    for (let r = first; r <= last; r++) {
      for (let c = 0; c < cols(); c++) {
        const i = r * cols() + c;
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
    if (i < 0 || rowH() <= 0) return;
    const margin = props.gap;
    const top = padTop() + Math.floor(i / cols()) * rowH() - margin;
    const bottom = padTop() + Math.floor(i / cols()) * rowH() + cardH() + margin;
    let target: number | null = null;
    if (top < el.scrollTop) target = Math.max(0, top);
    else if (bottom > el.scrollTop + height()) target = bottom - height();
    if (target !== null) {
      el.scrollTop = target;
      // Render the destination rows now rather than on the next scroll frame.
      setScrollTop(el.scrollTop);
    }
  });

  return (
    <div class="vgrid" ref={el} role="grid">
      <div class="vgrid-sizer" style={{ height: `${Math.max(0, rows() * rowH() - props.gap)}px` }}>
        <For each={visible()}>
          {(i) => {
            const x = () => (i % cols()) * (cardW() + props.gap);
            const y = () => Math.floor(i / cols()) * rowH();
            const isFocused = () => props.focused === i;
            return (
              <div
                class="vgrid-cell"
                role="gridcell"
                style={{
                  width: `${cardW()}px`,
                  height: `${cardH()}px`,
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
