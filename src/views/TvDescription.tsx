import { createMemo, createSignal, onCleanup, onMount } from "solid-js";
import { tvSummary } from "../state/summary";

/** A bounded TV synopsis, measured in the selected TV font. No polling or requests. */
export function TvDescription(props: { text: string; lines?: number }) {
  let el!: HTMLParagraphElement;
  let context: CanvasRenderingContext2D | null = null;
  const [metrics, setMetrics] = createSignal({ width: 0, font: "" });
  onMount(() => {
    context = document.createElement("canvas").getContext("2d");
    const measure = () => {
      const style = getComputedStyle(el);
      const next = { width: el.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight), font: `${style.fontWeight} ${style.fontSize} ${style.fontFamily}` };
      setMetrics((old) => old.width === next.width && old.font === next.font ? old : next);
    };
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    measure();
    onCleanup(() => observer.disconnect());
  });
  const summary = createMemo(() => {
    const { width, font } = metrics();
    if (!context || width <= 0) return tvSummary(props.text, (s) => s.length <= 160);
    context.font = font;
    return tvSummary(props.text, (text) => {
      let lines = 1;
      let line = "";
      for (const word of text.split(/\s+/)) {
        if (context!.measureText(word).width > width - 2) return false;
        const next = line ? `${line} ${word}` : word;
        if (line && context!.measureText(next).width > width - 2) {
          if (++lines > (props.lines ?? 6)) return false;
          line = word;
        } else line = next;
      }
      return true;
    });
  });
  return <p ref={el} class="tv-desc" style={{ "-webkit-line-clamp": props.lines ?? 6 }}>{summary()}</p>;
}
