// KB's sounds: menu moves, game starts and the start-up intros. Every built-in
// sound is mixed from CC0 recordings (tools/sounds); a user's own intro sound
// comes from data\intros. Played through Web Audio so they start on time.
import { fx } from "../state/fx";
import { session } from "../state/session";

const FILES = Object.fromEntries(
  Object.entries(import.meta.glob<string>("../assets/sounds/*.ogg", { eager: true, query: "?url", import: "default" })).map(
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

/** Plays a decoded sound now (or `delayMs` from now), if sound is on. */
export function playBuffer(buf: AudioBuffer | null, delayMs = 0) {
  const c = audio();
  if (!c || !buf || !master || !fx.soundOn() || session.running()) return;
  master.gain.value = fx.volume();
  const s = c.createBufferSource();
  s.buffer = buf;
  s.connect(master);
  s.start(c.currentTime + delayMs / 1000);
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

/** The current profile's sound for a menu move, a choice, going back, or a disc / cartridge game starting. */
export function sfx(role: "move" | "select" | "back" | "disc" | "cart", delayMs = 0) {
  play(`prof_${fx.get("soundProfile")}_${role}`, delayMs);
}

/** Decodes the menu sounds ahead of time, so the first press isn't late. */
export function warmUp() {
  if (!fx.soundOn()) return;
  for (const r of ["move", "select", "back"]) void load(`prof_${fx.get("soundProfile")}_${r}`);
}
