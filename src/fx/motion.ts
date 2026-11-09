import { fx } from "../state/fx";

/** Finite compositor motion; repeated input replaces the previous effect. */
export function moveIn(el: HTMLElement, frames: Keyframe[], duration = 300) {
  el.getAnimations().forEach((a) => a.cancel());
  if (fx.anim() === "off") return;
  el.animate(fx.anim() === "reduced" ? [{ opacity: 0 }, { opacity: 1 }] : frames, {
    duration: fx.anim() === "reduced" ? 140 : duration,
    easing: "cubic-bezier(.2,.8,.2,1)",
  });
}

export function growFrom(el: HTMLElement, source?: DOMRect) {
  if (!source || !source.width || !source.height) return;
  const target = el.getBoundingClientRect();
  if (!target.width || !target.height) return;
  moveIn(el, [
    { transformOrigin: "top left", transform: `translate(${source.x - target.x}px, ${source.y - target.y}px) scale(${source.width / target.width}, ${source.height / target.height})` },
    { transformOrigin: "top left", transform: "none" },
  ], 380);
}
