import { createRoot, createSignal } from "solid-js";
import { api, events, type PadInfo, type RunningGame } from "../api";
import { library } from "./library";

function createSessionStore() {
  const [running, setRunning] = createSignal<RunningGame | null>(null);
  const [lastPad, setLastPad] = createSignal<PadInfo | null>(null);
  // Why the last launch didn't run (the emulator closed right away), until shown.
  const [failure, setFailure] = createSignal<string | null>(null);

  function setPaused(paused: boolean) {
    // Stops CSS animations/transitions app-wide while a game runs.
    document.documentElement.classList.toggle("paused", paused);
  }

  /** Subscribes to game/controller events. Call once at startup. */
  function listen() {
    void events.onGameStarted((g) => {
      setRunning(g);
      setPaused(true);
    });
    void events.onGameExited((e) => {
      setRunning(null);
      setPaused(false);
      if (e.failed) setFailure(e.failed);
      void library.reload(); // play time / last played changed
    });
    void events.onGamepadConnected(setLastPad);
    // A game may already be running if the UI was reloaded.
    void api.getRunning().then((g) => {
      if (g) {
        setRunning(g);
        setPaused(true);
      }
    });
  }

  return { running, lastPad, failure, clearFailure: () => setFailure(null), listen };
}

export const session = createRoot(createSessionStore);

/** "3h 12m", "45m", "<1m" */
export function formatPlayTime(seconds: number): string {
  if (seconds < 60) return seconds > 0 ? "<1m" : "Never played";
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
}

export function formatLastPlayed(unixSeconds: number | null): string {
  if (!unixSeconds) return "Never";
  const d = new Date(unixSeconds * 1000);
  const days = Math.floor((Date.now() - d.getTime()) / 86_400_000);
  if (days === 0) return "Today";
  if (days === 1) return "Yesterday";
  if (days < 7) return `${days} days ago`;
  return d.toLocaleDateString();
}

/** Exit-combo button names → labels for the connected pad family. */
export function buttonLabel(name: string, family: PadInfo["family"] | undefined): string {
  const ps: Record<string, string> = {
    south: "Cross", east: "Circle", west: "Square", north: "Triangle",
    back: "Share/Create", start: "Options", guide: "PS", touchpad: "Touchpad",
    leftShoulder: "L1", rightShoulder: "R1", leftStick: "L3", rightStick: "R3",
  };
  const xbox: Record<string, string> = {
    south: "A", east: "B", west: "X", north: "Y",
    back: "View/Select", start: "Menu/Start", guide: "Xbox", touchpad: "Touchpad",
    leftShoulder: "LB", rightShoulder: "RB", leftStick: "LS", rightStick: "RS",
  };
  const dpad: Record<string, string> = { dpadUp: "D-pad Up", dpadDown: "D-pad Down", dpadLeft: "D-pad Left", dpadRight: "D-pad Right" };
  return dpad[name] ?? (family === "playstation" ? ps : xbox)[name] ?? name;
}

export const COMBO_BUTTONS = [
  "back", "start", "guide", "south", "east", "west", "north",
  "leftShoulder", "rightShoulder", "leftStick", "rightStick",
  "dpadUp", "dpadDown", "dpadLeft", "dpadRight", "touchpad",
];
