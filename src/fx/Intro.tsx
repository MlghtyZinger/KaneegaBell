import { createEffect, createRoot, createSignal, onCleanup, onMount, Show } from "solid-js";
import { api } from "../api";
import { pushHandler } from "../input/actions";
import { fx, INTROS, type IntroKey } from "../state/fx";
import { session } from "../state/session";
import { hush, load, playBuffer, readyAudio } from "./audio";
import "../styles/intro.css";

const FILES = import.meta.glob<string>("../assets/intros/*.mp4", { eager: true, query: "?url", import: "default" });
const KEYS = Object.keys(INTROS) as IntroKey[];
const videoUrl = (key: IntroKey) => FILES[`../assets/intros/${key === "rockstar" ? "rdr1" : key}.mp4`];

function pick(): IntroKey | null {
  const p = fx.startInTv() ? fx.get("introTv") : fx.get("introDesktop");
  if (p === "off" || session.running()) return null;
  // Retired choices (such as Xbox 360) fall back to the current lineup.
  return Object.hasOwn(INTROS, p) ? p as IntroKey : KEYS[Math.floor(Math.random() * KEYS.length)];
}

const runs = createRoot(() => {
  const [run, setRun] = createSignal<{ key: IntroKey | null } | null>({ key: null });
  return { run, setRun };
});
/** Menu music waits until the intro has finished or been skipped. */
export const introPlaying = () => runs.run() !== null;
export const playIntro = (key: IntroKey) => runs.setRun({ key });

export function Intros() {
  return <Show when={runs.run()} keyed>{(r) => <IntroHost forced={r.key} />}</Show>;
}

/** Reviewed clips keep their embedded audio and video on a single media clock. */
function IntroHost(props: { forced: IntroKey | null }) {
  const [key, setKey] = createSignal<IntroKey | null>(null);
  const [leaving, setLeaving] = createSignal(false);
  const [customSound, setCustomSound] = createSignal(false);
  let video: HTMLVideoElement | undefined;
  let disposed = false;
  let startedAt = 0;
  let buffer: AudioBuffer | null = null;
  let source: AudioBufferSourceNode | undefined;
  const timers: ReturnType<typeof setTimeout>[] = [];
  const at = (ms: number, f: () => void) => timers.push(setTimeout(f, ms));
  const finish = () => { if (!disposed) runs.setRun(null); };
  const leave = () => {
    if (disposed || leaving()) return;
    video?.pause();
    hush();
    setLeaving(true);
    at(300, finish);
  };
  const skip = () => key() ? leave() : finish();

  onCleanup(pushHandler(() => { skip(); return true; }));
  onCleanup(() => {
    disposed = true;
    timers.forEach(clearTimeout);
    video?.pause();
    source?.stop();
  });
  createEffect(() => {
    const selected = key();
    const muted = !fx.soundOn() || !!session.running() || customSound();
    const volume = fx.volume();
    if (selected && video) {
      video.muted = muted;
      video.volume = volume;
    }
  });

  onMount(async () => {
    if (!props.forced) await fx.load();
    if (disposed) return;
    const selected = props.forced ?? pick();
    if (!selected) return finish();
    if (fx.soundOn() && !session.running()) {
      const own = await api.introSound(selected).catch(() => null);
      if (disposed) return;
      buffer = own?.byteLength ? await load(`own_${selected}`, own) : null;
      if (disposed) return;
      setCustomSound(buffer !== null);
      if (buffer) await readyAudio();
    }
    if (!disposed) setKey(selected);
  });

  return (
    <Show when={key()} fallback={<div class="intro-wait" onClick={skip} />}>
      {(selected) => (
        <div class="intro" classList={{ leave: leaving() }} onClick={skip}>
          <video
            ref={(el) => { video = el; el.volume = fx.volume(); }}
            class="intro-video"
            src={videoUrl(selected())}
            preload="auto"
            playsinline
            muted={!fx.soundOn() || !!session.running() || customSound()}
            onLoadedData={() => {
              if (!disposed && !leaving()) void video?.play().catch(finish);
            }}
            onPlaying={() => {
              if (startedAt || disposed || leaving()) return;
              startedAt = performance.now();
              source = playBuffer(buffer);
            }}
            onEnded={() => {
              const remaining = (buffer?.duration ?? 0) * 1000 - (performance.now() - startedAt);
              if (remaining > 0) at(remaining, leave);
              else leave();
            }}
            onError={finish}
          />
        </div>
      )}
    </Show>
  );
}
