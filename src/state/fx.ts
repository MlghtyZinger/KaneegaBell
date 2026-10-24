import { createRoot, createSignal } from "solid-js";
import { api } from "../api";

/** How much the UI moves (Settings → General → Animations), desktop and TV Mode separately. */
export type AnimLevel = "full" | "reduced" | "off";
/** Start-up intro: one picked at random, a fixed one, or none. */
export type IntroPick = "random" | "off" | IntroKey;
export type SoundProfile = "tactile" | "console" | "retro";

/** The retained video intros, in the order Settings lists them. */
export const INTROS = {
  konami: { name: "Konami (Metal Gear)" },
  rockstar: { name: "Rockstar (Red Dead Redemption)" },
  gba: { name: "Game Boy Advance" },
  gamecube: { name: "GameCube" },
  ps1: { name: "PlayStation" },
  ps2: { name: "PlayStation 2" },
  ps3: { name: "PlayStation 3" },
  psp: { name: "PSP" },
  switch: { name: "Switch" },
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
  tvMenu: "kb" as "kb" | "wii" | "ps3",
  consoleFeedbackReady: false,
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
      for (const k of ["introDesktop", "introTv"] as const) {
        if (v[k] !== "random" && v[k] !== "off" && !Object.hasOwn(INTROS, v[k])) v[k] = "random";
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

/** Bumped when a back, spine or disc picture changes, so the 3D cases ask for it again (their URLs carry no version). */
const [caseRevision, setCaseRevision] = createRoot(() => createSignal(0));
export const caseRev = caseRevision;
export const bumpCaseRev = () => setCaseRevision((n) => n + 1);
