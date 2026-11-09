import { Ps2Background } from "./Ps2Background";
import { Ps3Background } from "./Ps3Background";
import { createEffect, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { desktopStyle } from "../state/theme";

/** Shared console backgrounds and subtle particles. Pause when hidden or unfocused. */
export function DesktopBackground(props: { style?: string } = {}) {
  const style = () => props.style ?? desktopStyle();
  const [active, setActive] = createSignal(!document.hidden && document.hasFocus());
  onMount(() => {
    const update = () => setActive(!document.hidden && document.hasFocus());
    document.addEventListener("visibilitychange", update);
    window.addEventListener("focus", update);
    window.addEventListener("blur", update);
    onCleanup(() => {
      document.removeEventListener("visibilitychange", update);
      window.removeEventListener("focus", update);
      window.removeEventListener("blur", update);
    });
  });
  return <div class="desktop-scenery" data-scenery={style()} classList={{ "scenery-paused": !active() }} aria-hidden="true">
    <Show when={style() === "forge"}>
      <div class="forge-embers"><For each={Array.from({ length: 22 }, (_, i) => i)}>{i => <i style={{ left: `${(i * 37 + 3) % 100}%`, "--delay": `${-i * .73}s`, top: `${15 + (i * 23) % 75}%`, "--duration": `${4 + i % 5}s`, "--drift": `${(i % 2 ? -1 : 1) * (30 + i * 3)}px` }} />}</For></div>
    </Show>
    <Show when={style() === "starlight"}>
      <div class="star-field"><For each={Array.from({ length: 90 }, (_, i) => i)}>{i => <i style={{ left: `${(i * 37.37 + 3) % 100}%`, top: `${(i * 23.17 + 7) % 100}%`, "--delay": `${-i * .73}s`, "--duration": `${2 + i % 5}s`, "--star-size": `${i % 9 === 0 ? 4 : i % 3 === 0 ? 2.5 : 1.5}px` }} />}</For></div>
    </Show>
    <Show when={style() === "obsidian"}><div class="obsidian-grid" /><div class="obsidian-scan" /></Show>
    <Show when={style() === "aurora"}><div class="aurora-curtain" /><div class="aurora-curtain aurora-second" /></Show>
    <Show when={style() === "ps3"}><Ps3Background /></Show>
    <Show when={style() === "ps2"}><Ps2Background active={active()} /></Show>
  </div>;
}

export function DesktopDock() {
  const [now, setNow] = createSignal(new Date());
  createEffect(() => {
    if (desktopStyle() !== "wii") return;
    setNow(new Date());
    const timer = setInterval(() => { if (!document.hidden) setNow(new Date()); }, 60_000);
    onCleanup(() => clearInterval(timer));
  });
  return <Show when={desktopStyle() === "wii"}>
    <div class="desktop-wii-dock">
      <span class="desktop-wii-word">Wii</span>
      <div class="desktop-wii-clock"><time>{now().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</time><span>{now().toLocaleDateString([], { weekday: "short", day: "numeric", month: "short" })}</span></div>
      <span class="desktop-wii-word">KB</span>
    </div>
  </Show>;
}
