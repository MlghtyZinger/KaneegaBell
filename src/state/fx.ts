import { createRoot, createSignal } from "solid-js";
import { api } from "../api";

/** How much the UI moves (Settings → General → Animations), desktop and TV Mode separately. */
export type AnimLevel = "full" | "reduced" | "off";
/** Start-up intro: one picked at random, a fixed one, or none. */
export type IntroPick = "random" | "off" | IntroKey;
export type SoundProfile = "tactile" | "console" | "retro";

/** The intros, in the order Settings lists them, with how long each runs (built-in sound / a sound file of your own). */
export const INTROS = {
  konami: { name: "Konami (Metal Gear)", ms: 2600, customMs: 4800 },
  rockstar: { name: "Rockstar (Red Dead 2)", ms: 3900, customMs: 7000 },
  gba: { name: "Game Boy Advance", ms: 2900, customMs: 4700 },
  gamecube: { name: "GameCube", ms: 3600, customMs: 5600 },
  ps1: { name: "PlayStation", ms: 4800, customMs: 7700 },
  ps2: { name: "PlayStation 2", ms: 4100, customMs: 6400 },
  ps3: { name: "PlayStation 3", ms: 4400, customMs: 6300 },
  psp: { name: "PSP", ms: 3800, customMs: 5600 },
  xbox360: { name: "Xbox 360", ms: 3800, customMs: 6300 },
  switch: { name: "Switch", ms: 2300, customMs: 2300 },
} as const;
export type IntroKey = keyof typeof INTROS;

/** Every setting this store owns, with its default (the owner's choices, 2026-10-20/21). */
const DEFAULTS = {
  animDesktop: "full" as AnimLevel,
  animTv: "full" as AnimLevel,
  introDesktop: "random" as IntroPick,
  introTv: "random" as IntroPick,
  launchFxDesktop: true,
  launchFxTv: true,
  tvShelf: "3d" as "3d" | "flat",
  soundDesktop: true,
  soundTv: true,
  volumeDesktop: 60,
  volumeTv: 60,
  soundProfile: "tactile" as SoundProfile,
};
type Fx = typeof DEFAULTS;

function createFx() {
  const [values, setValues] = createSignal<Fx>({ ...DEFAULTS });
  // Settles once the saved values are read (the start-up intro waits for it).
  const [loaded, setLoaded] = createSignal(false);
  const [startInTv, setStartInTv] = createSignal(false);
  const [tv, setTvRaw] = createSignal(false);

  const get = <K extends keyof Fx>(k: K): Fx[K] => values()[k];
  const anim = () => (tv() ? get("animTv") : get("animDesktop"));
  const launchFx = () => (tv() ? get("launchFxTv") : get("launchFxDesktop"));
  const soundOn = () => (tv() ? get("soundTv") : get("soundDesktop"));
  const volume = () => (tv() ? get("volumeTv") : get("volumeDesktop")) / 100;
  const shelf3d = () => get("tvShelf") === "3d" && get("animTv") !== "off";

  const mark = () => (document.documentElement.dataset.anim = anim());

  async function load() {
    try {
      const keys = Object.keys(DEFAULTS) as (keyof Fx)[];
      const stored = await api.getSettingValues([...keys, "startInTv"]);
      const v = { ...DEFAULTS };
      for (const k of keys) {
        const saved = stored[k];
        if (saved !== undefined && saved !== null && typeof saved === typeof DEFAULTS[k]) (v as Record<string, unknown>)[k] = saved;
      }
      setValues(v);
      setStartInTv(stored.startInTv === true);
    } catch {
      // Browser dev mock without settings: the defaults stand.
    }
    mark();
    setLoaded(true);
  }

  function set<K extends keyof Fx>(k: K, v: Fx[K]) {
    setValues((o) => ({ ...o, [k]: v }));
    mark();
    void api.setSetting(k, v).catch(() => {});
  }

  function setTv(on: boolean) {
    setTvRaw(on);
    mark();
  }

  return { get, set, load, loaded, startInTv, tv, setTv, anim, launchFx, soundOn, volume, shelf3d };
}

export const fx = createRoot(createFx);
