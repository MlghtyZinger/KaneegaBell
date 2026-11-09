import { createEffect, createSignal, onCleanup, onMount } from "solid-js";
import { fx } from "../state/fx";
import { session } from "../state/session";
import { introPlaying } from "../fx/Intro";
import { xmbHue, xmbWave } from "../fx/ps3Wave";
import "../styles/ps3-background.css";

/** One flowing XMB background for the console menu, desktop and KB TV theme. */
export function Ps3Background() {
  let canvas!: HTMLCanvasElement;
  const [hue, setHue] = createSignal(xmbHue(new Date()));
  const [active, setActive] = createSignal(!document.hidden && document.hasFocus());
  onMount(() => {
    let midnight: ReturnType<typeof setTimeout>;
    const updateDate = () => {
      clearTimeout(midnight);
      const now = new Date();
      setHue(xmbHue(now));
      const next = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1);
      midnight = setTimeout(updateDate, next.getTime() - now.getTime() + 50);
    };
    const visibility = () => { setActive(!document.hidden && document.hasFocus()); updateDate(); };
    document.addEventListener("visibilitychange", visibility);
    window.addEventListener("focus", visibility);
    window.addEventListener("blur", visibility);
    updateDate();
    onCleanup(() => {
      clearTimeout(midnight);
      document.removeEventListener("visibilitychange", visibility);
      window.removeEventListener("focus", visibility);
      window.removeEventListener("blur", visibility);
    });
    const ctx = canvas.getContext("2d", { alpha: false })!;
    const w = canvas.width, h = canvas.height;
    let time = 0;
    const draw = (color: number) => {
      const backdrop = ctx.createLinearGradient(0, 0, 0, h);
      backdrop.addColorStop(0, "#010104");
      backdrop.addColorStop(.48, `hsl(${color} 70% 5%)`);
      backdrop.addColorStop(1, `hsl(${color} 75% 13%)`);
      ctx.fillStyle = backdrop; ctx.fillRect(0, 0, w, h);
      ctx.globalCompositeOperation = "screen";
      for (let band = 0; band < 7; band++) {
        const upper: { x: number; y: number; width: number }[] = [];
        for (let step = 0; step <= 64; step++) {
          const x = step / 64, point = xmbWave(band, x, time);
          upper.push({ x: x * w, y: point.y * h, width: point.width * h });
        }
        ctx.beginPath();
        upper.forEach((p, i) => i ? ctx.lineTo(p.x, p.y) : ctx.moveTo(p.x, p.y));
        for (let i = upper.length - 1; i >= 0; i--) ctx.lineTo(upper[i].x, upper[i].y + upper[i].width);
        ctx.closePath();
        const sheen = ctx.createLinearGradient(0, h * .35, 0, h * .7);
        sheen.addColorStop(0, `hsla(${color} 60% 70% / .015)`);
        sheen.addColorStop(.5, `hsla(${color} 48% 73% / ${.06 + band * .007})`);
        sheen.addColorStop(1, `hsla(${color} 58% 66% / .02)`);
        ctx.fillStyle = sheen; ctx.fill();
        ctx.beginPath();
        upper.forEach((p, i) => i ? ctx.lineTo(p.x, p.y) : ctx.moveTo(p.x, p.y));
        ctx.strokeStyle = `hsla(${color} 35% 85% / ${band % 2 ? .12 : .055})`;
        ctx.lineWidth = band % 2 ? 1.1 : .7; ctx.stroke();
      }
      for (let i = 0; i < 110; i++) {
        const x = ((i * .618033 + time * (.002 + (i % 3) * .0004)) % 1) * w;
        const y = (((i * .414214 - time * .003) % 1 + 1) % 1) * h;
        const alpha = .15 + .55 * Math.pow(.5 + .5 * Math.sin(time * .8 + i * 2.3), 3);
        const r = i % 11 === 0 ? 1 : .55;
        ctx.fillStyle = `hsla(${color} 20% 95% / ${alpha})`;
        ctx.fillRect(x, y, r, r);
        if (i % 11 === 0) {
          ctx.fillStyle = `hsla(${color} 30% 90% / ${alpha * .3})`;
          ctx.fillRect(x - 2, y, 5, .6); ctx.fillRect(x, y - 2, .6, 5);
        }
      }
      ctx.globalCompositeOperation = "source-over";
    };
    createEffect(() => {
      const color = hue();
      const moving = active() && fx.anim() === "full" && !session.running() && !introPlaying();
      draw(color);
      if (!moving) return;
      let frame = 0, previous = performance.now();
      const animate = (now: number) => {
        time += Math.min(now - previous, 100) / 1000; previous = now;
        draw(color); frame = requestAnimationFrame(animate);
      };
      frame = requestAnimationFrame(animate);
      onCleanup(() => cancelAnimationFrame(frame));
    });
  });
  return <canvas ref={canvas} class="xmb-background" width="960" height="540" data-hue={hue()} aria-hidden="true" />;
}
