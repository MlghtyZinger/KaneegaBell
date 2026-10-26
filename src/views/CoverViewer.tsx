import { createSignal, onCleanup, Show } from "solid-js";
import { artUrl, events, type GameRow } from "../api";
import { pushHandler, type Action } from "../input/actions";
import { Badge } from "../components/Hints";
import { setMovesMuted } from "../fx/audio";
import { Model, caseOf } from "../fx/Model";
import { session } from "../state/session";
import { showcaseScale } from "./tvShowcase";

// shortcut: zoom stops at 16× (the case is laid out at that size, which gets heavy for the GPU); raise it if it stays smooth.
const MAX_ZOOM = 16;

/** A game's case filling the screen over its own blurred cover: turn it any way, flip it, zoom and move around. */
export function CoverViewer(props: { game: GameRow; platformName: string; onClose: () => void }) {
  const [v, setV] = createSignal({ ry: 0, rx: 0, zoom: 1, x: 0, y: 0, flipped: false, spring: true });
  const [view, setView] = createSignal({ w: innerWidth, h: innerHeight });
  const [logoFailed, setLogoFailed] = createSignal(false);
  const onResize = () => setView({ w: innerWidth, h: innerHeight });
  addEventListener("resize", onResize);
  onCleanup(() => removeEventListener("resize", onResize));

  const scale = () => {
    const [w, h] = caseOf(props.game.platformId).size;
    const base = showcaseScale(view().w, view().h * 0.9, w, h, props.game.art > 0 ? props.game.artAspect : null);
    return base * v().zoom;
  };
  // Zoom glides toward its target, so held triggers and the wheel zoom smoothly instead of in notches.
  let target = 1;
  let raf = 0;
  const glide = () => {
    const z = v().zoom;
    const next = Math.abs(target - z) < target * 0.002 ? target : z + (target - z) * 0.2;
    setV((s) => ({ ...s, zoom: next }));
    raf = next === target ? 0 : requestAnimationFrame(glide);
  };
  const zoomTo = (t: number) => {
    target = Math.min(MAX_ZOOM, Math.max(1, t));
    if (!raf) raf = requestAnimationFrame(glide);
  };
  const zoom = (f: number) => zoomTo(target * f);
  onCleanup(() => cancelAnimationFrame(raf));
  setMovesMuted(true);
  onCleanup(() => setMovesMuted(false));
  const move = (dx: number, dy: number) => setV((s) => ({ ...s, spring: true, x: s.x + dx * view().w * 0.08, y: s.y + dy * view().h * 0.08 }));
  const reset = () => {
    setV((s) => ({ ...s, ry: 0, rx: 0, x: 0, y: 0, flipped: false, spring: true }));
    zoomTo(1);
  };
  const flip = () => setV((s) => ({ ...s, spring: true, flipped: !s.flipped }));

  // Right stick: free turning both ways, one sample per frame; its own step actions are ignored meanwhile.
  // Any stick sample, centred or not, blocks the step actions for a moment: a step queued on one side
  // could otherwise land after the stick crossed to the other side and turn the case back.
  let stickAt = 0;
  let movedAt = 0;
  const offStick = events.onRightStick(([x, y]) => {
    if (session.running()) return;
    const now = performance.now();
    const dt = movedAt && now - movedAt < 100 ? (now - movedAt) / 1000 : 1 / 60;
    stickAt = now;
    movedAt = x || y ? now : 0;
    if (x || y) setV((s) => ({ ...s, spring: false, ry: s.ry + x * 220 * dt, rx: s.rx - y * 160 * dt }));
  });
  onCleanup(() => void offStick.then((off) => off()).catch(() => {}));
  const step = (dy: number, dx: number) =>
    setTimeout(() => performance.now() - stickAt > 300 && setV((s) => ({ ...s, spring: true, ry: s.ry + dy, rx: s.rx + dx })), 40);

  onCleanup(
    pushHandler((a: Action) => {
      switch (a) {
        case "back": props.onClose(); break;
        case "select": reset(); break;
        case "prevLetter": zoom(1 / 1.25); break;
        case "nextLetter": zoom(1.25); break;
        case "left": move(1, 0); break;
        case "right": move(-1, 0); break;
        case "up": move(0, 1); break;
        case "down": move(0, -1); break;
        case "turnLeft": step(-20, 0); break;
        case "turnRight": step(20, 0); break;
        case "zoomIn": step(0, 15); break;
        case "zoomOut": step(0, -15); break;
        case "toggleGroups": flip(); break;
      }
      return true; // modal
    }),
  );

  // Mouse: drag turns, the wheel zooms.
  let drag: { x: number; y: number; ry: number; rx: number } | null = null;
  const onMove = (e: PointerEvent) => {
    if (drag) setV((s) => ({ ...s, spring: false, ry: drag!.ry + (e.clientX - drag!.x) * 0.5, rx: drag!.rx - (e.clientY - drag!.y) * 0.5 }));
  };
  const onUp = () => (drag = null);
  addEventListener("pointermove", onMove);
  addEventListener("pointerup", onUp);
  onCleanup(() => {
    removeEventListener("pointermove", onMove);
    removeEventListener("pointerup", onUp);
  });

  return (
    <div class="cv" onWheel={(e) => zoom(e.deltaY < 0 ? 1.15 : 1 / 1.15)} onPointerDown={(e) => (drag = { x: e.clientX, y: e.clientY, ry: v().ry, rx: v().rx })}>
      {props.game.art > 0 && <img class="cv-bg" src={artUrl(props.game.id, props.game.art, "f")} alt="" />}
      <div class="cv-stage" style={{ transform: `translate(${v().x}px, ${v().y}px)`, transition: v().spring ? "transform .3s ease-out" : "none" }}>
        <div
          class="m3-turn"
          style={{ transform: `rotateX(${v().rx}deg) rotateY(${v().ry + (v().flipped ? 180 : 0)}deg)`, transition: v().spring ? "transform .45s cubic-bezier(.2,.8,.2,1)" : "none" }}
        >
          <Model game={props.game} platformName={props.platformName} scale={scale()} detail hires />
        </div>
      </div>
      {/* The game's logo (owner's trial, 2026-10-26); the title when it has none or the file is gone. */}
      <header class="cv-title">
        <Show when={props.game.logo && !logoFailed()} fallback={<h1>{props.game.title}</h1>}>
          <img class="cv-logo" src={artUrl(props.game.id, props.game.logo!, "l")} alt={props.game.title} onError={() => setLogoFailed(true)} />
        </Show>
      </header>
      <div class="cv-chips">
        <button onClick={flip}>{v().flipped ? "Back" : "Front"}<Badge action="toggleGroups" /></button>
        <button onClick={() => zoom(1 / 1.25)} aria-label="Zoom out"><Badge action="prevLetter" />−</button>
        <span>{Math.round(v().zoom * 100)}%</span>
        <button onClick={() => zoom(1.25)} aria-label="Zoom in">+<Badge action="nextLetter" /></button>
        <button onClick={reset}>Reset<Badge action="select" /></button>
        <button onClick={props.onClose}>Close<Badge action="back" /></button>
      </div>
    </div>
  );
}
