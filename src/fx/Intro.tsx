import { createEffect, createRoot, createSignal, Match, onCleanup, onMount, Show, Switch } from "solid-js";
import { Stage } from "./Stage";
import { api } from "../api";
import { pushHandler } from "../input/actions";
import { fx, INTROS, type IntroKey } from "../state/fx";
import { session } from "../state/session";
import { hush, load, playBuffer, readyAudio } from "./audio";
import { emblem } from "./emblem";
import "../styles/intro.css";

const KEYS = Object.keys(INTROS) as IntroKey[];
const LETTERS = [..."KaneegaBell"];

/** Which intro plays at start-up: the setting for the mode KB starts in. */
function pick(): IntroKey | null {
  const p = fx.startInTv() ? fx.get("introTv") : fx.get("introDesktop");
  if (p === "off" || session.running()) return null;
  return p === "random" ? KEYS[Math.floor(Math.random() * KEYS.length)] : (p as IntroKey);
}

/** The intro to show: the start-up one (`key` null: the setting decides), or one asked for in Settings. */
const runs = createRoot(() => {
  const [run, setRun] = createSignal<{ key: IntroKey | null } | null>({ key: null });
  return { run, setRun };
});
/** Menu music waits until the intro has finished or been skipped. */
export const introPlaying = () => runs.run() !== null;
/** Plays an intro now (Settings → General → Start-up intro). */
export const playIntro = (key: IntroKey) => runs.setRun({ key });

export function Intros() {
  return (
    <Show when={runs.run()} keyed>
      {(r) => <IntroHost forced={r.key} />}
    </Show>
  );
}

/**
 * The start-up intro: covers the window until the saved settings are read, then
 * plays the chosen intro (with the user's own sound file for it from data\intros,
 * if there is one) and gets out of the way. Any button, key or click skips it.
 */
function IntroHost(props: { forced: IntroKey | null }) {
  const [state, setState] = createSignal<"wait" | "play" | "done">("wait");
  const [key, setKey] = createSignal<IntroKey | null>(null);
  const [on, setOn] = createSignal(false);
  const [leaving, setLeaving] = createSignal(false);
  let disposed = false;
  let frame = 0;
  let source: AudioBufferSourceNode | undefined;
  const timers: ReturnType<typeof setTimeout>[] = [];
  const at = (ms: number, f: () => void) => timers.push(setTimeout(f, ms));
  onCleanup(() => {
    disposed = true;
    cancelAnimationFrame(frame);
    timers.forEach(clearTimeout);
    source?.stop();
  });

  const finish = () => {
    timers.forEach(clearTimeout);
    setState("done");
    runs.setRun(null);
  };
  const skip = () => {
    if (state() === "wait") return finish();
    if (state() !== "play" || leaving()) return;
    hush();
    setLeaving(true);
    at(300, finish);
  };
  onCleanup(pushHandler(() => {
    if (state() === "done") return false;
    skip();
    return true;
  }));

  onMount(async () => {
    if (!props.forced) await fx.load();
    if (disposed) return;
    const k = props.forced ?? pick();
    if (!k) return finish();
    setKey(k);
    // The user's own sound for this intro, if any; else the built-in one.
    let buf: AudioBuffer | null = null;
    if (fx.soundOn() && !session.running()) {
      const own = await api.introSound(k).catch(() => null);
      buf = own && own.byteLength > 0 ? await load(`own_${k}`, own) : null;
      buf ??= await load(`intro_${k}`);
      await readyAudio();
    }
    if (disposed) return;
    setState("play");
    const ms = Math.max(INTROS[k].customMs, (buf?.duration ?? 0) * 1000);
    frame = requestAnimationFrame(() => {
      if (disposed) return;
      source = playBuffer(buf);
      setOn(true);
      at(ms, () => setLeaving(true));
      at(ms + 360, finish);
    });
  });

  return (
    <Show when={state() !== "done"}>
      <Show when={state() === "play"} fallback={<div class="intro-wait" />}>
        <div class="intro custom" classList={{ on: on(), leave: leaving() }} onClick={skip}>
          <Stage class="intro-stage">
            <Switch>
              <Match when={key() === "konami"}>
                <div class="iv konami">
                  <div class="kwm"><span class="kw">KaneegaBell</span><span class="kwf">KaneegaBell</span><div class="shine" /></div>
                  <div class="kseg">
                    <span style={{ "--t": ".25s" }} /><span style={{ "--t": ".43s" }} /><span style={{ "--t": ".61s" }} /><span style={{ "--t": ".79s" }} />
                  </div>
                </div>
              </Match>
              <Match when={key() === "rockstar"}>
                <Rockstar playing={on()} at={at} />
              </Match>
              <Match when={key() === "gba"}>
                <div class="iv gba">
                  <div class="gbaw">
                    {LETTERS.map((c, i) => <span style={{ "--dl": `${0.05 + i * 0.07}s` }}>{c}</span>)}
                    <div class="sweep" />
                  </div>
                  <div class="gbasub">ADVANCE</div>
                </div>
              </Match>
              <Match when={key() === "gamecube"}>
                <div class="iv gamecube">
                  <div class="gcp">
                    <i class="tr t1" /><i class="tr t2" /><i class="tr t3" /><i class="tr t4" /><i class="tr t5" />
                    <i class="gcb" />
                    <div class="emb" innerHTML={emblem("indigo")} />
                  </div>
                  <div class="gcname">KaneegaBell<small>GAMECUBE</small></div>
                </div>
              </Match>
              <Match when={key() === "ps1"}>
                <div class="iv ps1">
                  <div class="ps1a">
                    <div class="dia"><span /><span style={{ "--d": ".3s" }} /><span style={{ "--d": ".4s" }} /><span style={{ "--d": ".5s" }} /></div>
                    <div class="sce">KaneegaBell<small>Computer Entertainment</small></div>
                  </div>
                  <div class="ps1b">
                    <div class="emb" innerHTML={emblem()} />
                    <div class="lic">LICENSED BY</div>
                    <div class="lname">KaneegaBell</div>
                  </div>
                </div>
              </Match>
              <Match when={key() === "ps2"}>
                <div class="iv ps2">
                  <div class="towers">
                    {Array.from({ length: 36 }, () => (
                      <i style={{ "--x": `${Math.random() * 96}%`, "--w": `${6 + Math.random() * 16}px`, "--h": `${50 + Math.random() * 240}px`, "--z": `${-320 + Math.random() * 420}px`, "--dl": `${Math.random() * 1.8}s` }} />
                    ))}
                  </div>
                  <div class="motes">
                    {Array.from({ length: 40 }, () => (
                      <i style={{ "--x": `${Math.random() * 100}%`, "--dl": `${Math.random() * 2.5}s`, "--md": `${2.5 + Math.random() * 2.5}s` }} />
                    ))}
                  </div>
                  <div class="fog" />
                  <div class="p2black" />
                  <div class="p2name">KANEEGABELL</div>
                </div>
              </Match>
              <Match when={key() === "ps3"}>
                <div class="iv ps3">
                  <div class="waves"><i class="w1" /><i class="w2" /><i class="w3" /></div>
                  <div class="p3glow" />
                  <div class="p3name">KaneegaBell</div>
                </div>
              </Match>
              <Match when={key() === "psp"}>
                <div class="iv psp">
                  <div class="pblue" />
                  <div class="pline" />
                  <svg class="pwave" viewBox="0 0 960 200" preserveAspectRatio="none" aria-hidden="true">
                    <path d="M0 120 C 160 60, 320 170, 480 110 S 800 60, 960 120" />
                    <path class="pw2" d="M0 140 C 200 90, 360 190, 520 130 S 820 80, 960 140" />
                    <path class="pw3" d="M0 100 C 140 50, 300 150, 470 95 S 780 40, 960 100" />
                  </svg>
                  <div class="pflare" />
                  <div class="pwhite" />
                  <div class="pspname">
                    KaneegaBell<span class="prefl" aria-hidden="true">KaneegaBell</span>
                  </div>
                </div>
              </Match>
              <Match when={key() === "xbox360"}>
                <div class="iv xbox">
                  <div class="ribbons"><i /><i /><i /><i /><i /></div>
                  <div class="xx"><i /><i /></div>
                  <div class="xorb"><div class="emb" innerHTML={emblem("green")} /></div>
                  <div class="xbflash" />
                  <div class="xbname">KANEEGABELL</div>
                </div>
              </Match>
              <Match when={key() === "switch"}>
                <div class="iv nsw">
                  <div class="jc"><i class="jl" /><i class="jr" /></div>
                  <div class="swname">KaneegaBell</div>
                </div>
              </Match>
            </Switch>
          </Stage>
        </div>
      </Show>
    </Show>
  );
}

/**
 * Rockstar (Red Dead Redemption 2): dusk, the sight settles on the emblem and
 * it's shot: a spread of pellet holes at the blast in the owner's recording.
 */
function Rockstar(props: { playing: boolean; at: (ms: number, f: () => void) => void }) {
  let holes!: HTMLDivElement;
  let target!: HTMLDivElement;
  createEffect(() => {
    if (!props.playing) return;
    props.at(5080, () => {
      const pellets = 9;
      const cx = 30 + Math.random() * 40;
      const cy = 30 + Math.random() * 40;
      for (let p = 0; p < pellets; p++) {
        const x = pellets > 1 ? cx + (Math.random() - 0.5) * 34 : 18 + Math.random() * 64;
        const y = pellets > 1 ? cy + (Math.random() - 0.5) * 34 : 18 + Math.random() * 64;
        const h = document.createElement("span");
        h.className = pellets > 1 ? "bh small" : "bh";
        h.style.left = `${x}%`;
        h.style.top = `${y}%`;
        holes.appendChild(h);
        for (let c = 0; c < (pellets > 1 ? 2 : 7); c++) {
          const ch = document.createElement("span");
          const a = Math.random() * Math.PI * 2;
          const r = 30 + Math.random() * 60;
          ch.className = "chip";
          ch.style.cssText = `left:${x}%;top:${y}%;--cx:${Math.cos(a) * r}px;--cy:${Math.sin(a) * r}px`;
          holes.appendChild(ch);
        }
      }
      target.className = "tgt-in";
      void target.offsetWidth; // restart the jolt
      target.className = "tgt-in knock";
    });
  });
  return (
    <div class="iv rockstar">
      <div class="ridge" />
      <div class="dust">
        {Array.from({ length: 16 }, () => (
          <i style={{ "--y": `${10 + Math.random() * 70}%`, "--s": `${2 + Math.random() * 3}px`, "--dl": `${Math.random() * 1.5}s`, "--md": `${2.5 + Math.random() * 3}s` }} />
        ))}
      </div>
      <div class="reticle" />
      <div class="target">
        <div class="tgt-in" ref={target}>
          <div class="emb" innerHTML={emblem()} />
          <div ref={holes} />
        </div>
      </div>
      <div class="rname">KaneegaBell</div>
    </div>
  );
}
