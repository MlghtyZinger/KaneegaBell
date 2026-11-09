import { createEffect, on, onCleanup, onMount } from "solid-js";
import { fx } from "../state/fx";
import { session } from "../state/session";
import { introPlaying } from "../fx/Intro";
import "../styles/ps2-background.css";

const smooth = (x: number) => { x = Math.max(0, Math.min(1, x)); return x * x * (3 - 2 * x); };
/** A slow drift that never settles into a loop: two sines at unrelated rates, about -1..1. */
const drift = (t: number, a: number, b: number, p: number, q: number) => Math.sin(a * t + p) * .6 + Math.sin(b * t + q) * .4;
/**
 * Where orb `i` is at time `t`, in the intro's 1920x1080 space, relative to the ring's centre. The menu opens exactly on the
 * intro's ring, then the lights drift: the gap between them, how flat the path is, its size and its tilt each wander on their
 * own unrelated rhythms, so the lights sometimes circle, trail as an arc, line up and spin as a row, or bunch into one comet,
 * and everything in between, never in the same order. `sp` (the gap between neighbours) tells the comet from the rest.
 */
export const orbAt = (i: number, t: number) => {
  const ft = t - 8.8, u = t - 5.15, w = smooth(ft / 3);
  const ring = { sp: Math.PI / 4, flat: .72 + .08 * Math.sin(u * .6), R: 135, rot: 0, ph: u * 1.3 };
  const free = { sp: .04 + .76 * (.5 + .5 * drift(ft, .29, .171, 1.3, .4)), flat: .06 + .8 * (.5 + .5 * drift(ft, .23, .137, 2.1, 0)),
    R: 125 + 40 * drift(ft, .19, .097, .7, 2.2), rot: .21 * ft + .8 * Math.sin(.13 * ft), ph: u * 1.3 + .9 * Math.sin(.17 * u + .3) };
  const m = (k: keyof typeof ring) => ring[k] * (1 - w) + free[k] * w;
  const a = m("ph") + i * m("sp"), R = m("R"), fl = m("flat"), rot = m("rot");
  const lx = Math.cos(a) * R + 7 * w * Math.sin(u * 1.1 + i * 1.7), ly = Math.sin(a) * R * fl + 7 * w * Math.cos(u * .9 + i * 2.3);
  return { x: lx * Math.cos(rot) - ly * Math.sin(rot), y: lx * Math.sin(rot) + ly * Math.cos(rot), a, sp: m("sp") };
};

/** Original PS2 menu ring, shared unchanged with desktop and KB TV. */
export function Ps2Background(props: { focused?: number; launching?: boolean; active?: boolean }) {
  let canvas!: HTMLCanvasElement;
  /** Extra ring speed after the selection moves; eases back to 0. */
  let boost = 0;
  createEffect(on(() => props.focused, () => { boost = Math.min(3, boost + 1.6); }, { defer: true }));
  onMount(() => {
    // Same final-ring coordinates and phase as the approved canvas intro; bounded work.
    // KB's own animation level decides (Windows' "Show animations" is ignored, as everywhere in KB).
    // 854x480: the intro's own softened 16:9 resolution, so its last frame and the menu's first frame match.
    const ctx = canvas.getContext("2d", { alpha: false })!;ctx.scale(854/1920,480/1080);
    let frame = 0, time = 8.8, previous = performance.now(), wasIntro = introPlaying(), pull = 0;
    const draw = () => {
      ctx.fillStyle = "#000"; ctx.fillRect(0, 0, 1920, 1080);
      const glow = (x: number, y: number, radius: number, color: string, alpha: number) => {
        ctx.globalAlpha = alpha; const g = ctx.createRadialGradient(x, y, 0, x, y, radius);
        g.addColorStop(0, color); g.addColorStop(1, "transparent"); ctx.fillStyle = g; ctx.fillRect(x-radius, y-radius, radius*2, radius*2);
      };
      ctx.globalCompositeOperation = "screen";
      // When the lights bunch into a comet its tail is longer: the trail's samples are spaced further apart in time.
      const gap = .02 + .04 * (1 - smooth(orbAt(0, time).sp / .35)), keep = 1 - .9 * pull;
      for (let i = 0; i < 8; i++) for (let tail = 9; tail >= 0; tail--) {
        const t = time - tail*gap, o = orbAt(i, t), d = .85 + .15 * Math.sin(o.a), x = 780 + o.x * keep, y = 510 + o.y * keep;
        glow(x,y,38*d,"#37b5ff",(1-tail/10)*d*.3);
        if (!tail) { glow(x,y,19*d,"#37b5ff",.9); glow(x,y,12*d,"#d7f9ff",1); ctx.globalAlpha=1;ctx.fillStyle="#f7ffff";ctx.beginPath();ctx.ellipse(x,y,8*d,6.5*d,0,0,Math.PI*2);ctx.fill(); }
      }
      if (pull > 0) glow(780,510,60+140*pull,"#d7f9ff",pull*.9);
      ctx.globalAlpha=1;ctx.globalCompositeOperation="source-over";
    };
    let timer: ReturnType<typeof setTimeout> | undefined, painted=-1;
    const animate = (now: number) => {
      const playing=introPlaying();if(wasIntro&&!playing)time=8.8;wasIntro=playing;
      const moving=fx.anim()==='full'&&props.active!==false&&!document.hidden&&!session.running()&&!playing;
      if(moving){const step=Math.min(now-previous,100)/1000;
        // Moving the selection gives the ring a brief push that eases off; starting a game pulls the orbs into one bright point.
        boost*=Math.exp(-step*2.2); pull=Math.max(0,Math.min(1,pull+(props.launching?step*1.4:-step*2.5)));
        time+=step*(1+boost);}
      if(!document.hidden&&!session.running()&&!playing&&painted!==time){draw();painted=time;}
      previous=now;
      if(moving)frame=requestAnimationFrame(animate);else timer=setTimeout(()=>animate(performance.now()),100);
    };
    draw();frame=requestAnimationFrame(animate);onCleanup(()=>{cancelAnimationFrame(frame);clearTimeout(timer);});
  });
  return <canvas ref={canvas} class="ps2-ring" width="854" height="480" aria-hidden="true" />;
}
