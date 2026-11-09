// Menu and launch sources are documented in assets/sounds/menu-sources.md.
import { createSignal } from "solid-js";
import { fx } from "../state/fx";
import { session } from "../state/session";
import { desktopStyle } from "../state/theme";
import { soundName, soundProfile, type SoundRole } from "./soundProfile";

const FILES = Object.fromEntries(
  Object.entries(import.meta.glob<string>(["../assets/sounds/*.ogg", "!../assets/sounds/intro_*.ogg"], { eager: true, query: "?url", import: "default" })).map(
    ([path, url]) => [path.split("/").pop()!.replace(".ogg", ""), url],
  ),
);

let ctx: AudioContext | null = null;
let master: GainNode | null = null;
const buffers = new Map<string, Promise<AudioBuffer | null>>();

function audio(): AudioContext | null {
  if (!ctx) {
    try {
      ctx = new AudioContext();
    } catch {
      return null;
    }
    wire();
  }
  if (ctx.state === "suspended") void ctx.resume();
  return ctx;
}

function wire() {
  master = ctx!.createGain();
  master.gain.value = fx.volume();
  master.connect(ctx!.destination);
}

/** Decodes a sound once (built-in name, or bytes of the user's file under `key`). */
export function load(name: string, bytes?: ArrayBuffer): Promise<AudioBuffer | null> {
  let p = buffers.get(name);
  if (!p) {
    const c = audio();
    if (!c) return Promise.resolve(null);
    const src = bytes ? Promise.resolve(bytes) : FILES[name] ? fetch(FILES[name]).then((r) => r.arrayBuffer()) : null;
    if (!src) return Promise.resolve(null);
    p = src.then((b) => c.decodeAudioData(b)).catch(() => null);
    buffers.set(name, p);
  }
  return p;
}

/** Await before starting a visual timeline: a suspended device must not delay its audio. */
export async function readyAudio(): Promise<void> {
  const c = audio();
  if (c?.state === "suspended") await c.resume().catch(() => {});
}

/** Plays a decoded sound now (or `delayMs` from now), if sound is on and no game is running. */
export function playBuffer(buf: AudioBuffer | null, delayMs = 0, launch = false) {
  const c = audio();
  if (!c || !buf || !master || !fx.soundOn() || (session.running() && !(launch && document.documentElement.classList.contains("launch-presenting")))) return;
  master.gain.value = fx.volume();
  const s = c.createBufferSource();
  s.buffer = buf;
  s.connect(master);
  s.start(c.currentTime + delayMs / 1000);
  return s;
}

export function play(name: string, delayMs = 0) {
  if (!fx.soundOn()) return;
  void load(name).then((b) => playBuffer(b, delayMs));
}

/** Cuts every sound that's playing (an intro skipped, a launch cancelled). */
export function hush() {
  if (!ctx || !master) return;
  const old = master;
  old.gain.setTargetAtTime(0, ctx.currentTime, 0.03);
  setTimeout(() => old.disconnect(), 400);
  wire();
}

/** The cover viewer moves the picture with the d-pad: no move ticks while it's open. */
export const [movesMuted, setMovesMuted] = createSignal(false);

let lastCue = "";
let lastCueAt = 0;

const profile = () => soundProfile(fx.tv(), fx.get("tvMenu"), desktopStyle(), fx.get("menuSounds"));

export function sfx(role: SoundRole, delayMs = 0) {
  if (!fx.soundOn()) return;
  const name = soundName(profile(), role);
  if (!name) return;
  const now = performance.now();
  if (lastCue === name && now - lastCueAt < 35) return;
  lastCue = name; lastCueAt = now;
  const launch = role === "cart" || role === "disc";
  void load(name).then((b) => playBuffer(b, delayMs, launch));
}

/** Decodes the menu sounds ahead of time, so the first press isn't late. */
export function warmUp() {
  const style = profile();
  if (!fx.soundOn()) return;
  for (const r of ["move", "select", "back", "error", "cart", "disc"] as const) {
    const name = soundName(style, r);
    if (name) void load(name);
  }
}

export async function previewMenuSounds() {
  if (!fx.soundOn()) return;
  const style = fx.get("menuSounds");
  const clips = await Promise.all(["move", "select", "back"].map((r) => load(`menu_${style}_${r}`)));
  clips.forEach((clip, i) => playBuffer(clip, [0, 400, 1500][i]));
}
