import { createEffect, createRoot, createSignal, onCleanup, onMount, Show } from "solid-js";
import { api } from "../api";
import { pushHandler } from "../input/actions";
import { fx, INTROS, introsOffCached, type IntroKey } from "../state/fx";
import { session } from "../state/session";
import { hush, load, playBuffer, readyAudio } from "./audio";
import "../styles/intro.css";
import { preparePs2Audio, startPs2Intro, endPs2Intro } from "./ps2Audio";

const FILES = import.meta.glob<string>("../assets/intros/*.mp4", { eager: true, query: "?url", import: "default" });
const KEYS = Object.keys(INTROS) as IntroKey[];
const videoUrl = (key: IntroKey) => FILES[`../assets/intros/${key === "rockstar" ? "rdr1" : key}.mp4`];

function pick(tv: boolean): IntroKey | null {
  const p = tv ? fx.get("introTv") : fx.get("introDesktop");
  if (p === "off" || session.running()) return null;
  // The PS2 menu style opens with its own intro, which hands its orb ring straight to the menu.
  if (tv && fx.get("tvMenu") === "ps2") return "ps2";
  // Retired choices (such as Xbox 360) fall back to the current lineup.
  if (Object.hasOwn(INTROS, p)) return p as IntroKey;
  // Random: skip the one played last (unless it's the only one).
  const last = lastIntro();
  const pool = KEYS.length > 1 ? KEYS.filter((k) => k !== last) : KEYS;
  const next = pool[Math.floor(Math.random() * pool.length)];
  try { localStorage.setItem("kb.lastIntro", next); } catch { /* private mode: repeats are possible */ }
  return next;
}

function lastIntro(): string | null {
  try { return localStorage.getItem("kb.lastIntro"); } catch { return null; }
}

const runs = createRoot(() => {
  // `tv`: pick by the TV Mode setting (else by where KB opened).
  // Nothing covers the library while the settings load when both intros were off last time.
  const [run, setRun] = createSignal<{ key: IntroKey | null; tv?: boolean } | null>(introsOffCached() ? null : { key: null });
  return { run, setRun };
});
/** Menu music waits until the intro has finished or been skipped. */
export const introPlaying = () => runs.run() !== null;
export const playIntro = (key: IntroKey) => runs.setRun({ key });
/** Switching into TV Mode plays its start-up intro (owner, 2026-10-27), unless one is playing already. */
export const playTvIntro = () => {
  if (!runs.run()) runs.setRun({ key: null, tv: true });
};

export function Intros() {
  return <Show when={runs.run()} keyed>{(r) => <IntroHost forced={r.key} tv={r.tv} />}</Show>;
}

/** Reviewed clips keep their embedded audio and video on a single media clock. */
function IntroHost(props: { forced: IntroKey | null; tv?: boolean }) {
  const [key, setKey] = createSignal<IntroKey | null>(null);
  const [leaving, setLeaving] = createSignal(false);
  const [blackOut, setBlackOut] = createSignal(false);
  const [customSound, setCustomSound] = createSignal(false);
  let video: HTMLVideoElement | undefined;
  let disposed = false;
  let startedAt = 0;
  let syncedPs2 = false;
  let buffer: AudioBuffer | null = null;
  let source: AudioBufferSourceNode | undefined;
  const timers: ReturnType<typeof setTimeout>[] = [];
  const at = (ms: number, f: () => void) => timers.push(setTimeout(f, ms));
  const finish = () => { if (!disposed) runs.setRun(null); };
  const leave = () => {
    if (disposed || leaving() || blackOut()) return;
    const natural = video?.ended ?? false;
    video?.pause();
    if (syncedPs2) endPs2Intro(natural);
    hush();
    const reveal = () => { setLeaving(true); at(300, finish); };
    if (key() === "ps2" && !(fx.tv() && fx.get("tvMenu") === "ps2")) {
      // Fade the picture against the opaque black host before revealing another style.
      setBlackOut(true);
      at(350, reveal);
    } else reveal();
  };
  const skip = () => key() ? leave() : finish();

  onCleanup(pushHandler(() => { skip(); return true; }, true));
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
    const selected = props.forced ?? pick(props.tv ?? fx.startInTv());
    if (!selected) return finish();
    if (fx.soundOn() && !session.running()) {
      const own = await api.introSound(selected).catch(() => null);
      if (disposed) return;
      buffer = own?.byteLength ? await load(`own_${selected}`, own) : null;
      if (disposed) return;
      setCustomSound(buffer !== null);
      if (buffer) await readyAudio();
    }
    if (selected === "ps2" && !buffer && fx.tv() && fx.get("tvMenu") === "ps2") {
      try { await preparePs2Audio(); syncedPs2 = true; setCustomSound(true); } catch { /* embedded audio remains available */ }
    }
    if (!disposed) setKey(selected);
  });

  return (
    <Show when={key()} fallback={<div class="intro-wait" onClick={skip} />}>
      {(selected) => (
        <div class="intro" classList={{ leave: leaving(), "black-out": blackOut() }} onClick={skip}>
          <video
            ref={(el) => { video = el; el.volume = fx.volume(); }}
            class="intro-video"
            classList={{ fill: selected() === "ps2" }}
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
              if (syncedPs2) void startPs2Intro();
              else source = playBuffer(buffer);
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
