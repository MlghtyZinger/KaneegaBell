import { createSignal, onCleanup, type JSX } from "solid-js";

/**
 * A 540 px tall stage as wide as the window, scaled to its height: the intros
 * and the launch animation are drawn in these units at any resolution.
 */
export function Stage(props: { class: string; children: JSX.Element }) {
  const [size, setSize] = createSignal({ w: innerWidth, h: innerHeight });
  const onResize = () => setSize({ w: innerWidth, h: innerHeight });
  addEventListener("resize", onResize);
  onCleanup(() => removeEventListener("resize", onResize));
  return (
    <div class={props.class} style={{ width: `${(540 * size().w) / size().h}px`, "--s": String(size().h / 540) }}>
      {props.children}
    </div>
  );
}
