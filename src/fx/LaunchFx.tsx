import { createEffect, createRoot, createSignal, onCleanup, Show } from "solid-js";
import type { GameRow } from "../api";
import { pushHandler } from "../input/actions";
import { fx } from "../state/fx";
import { library } from "../state/library";
import { session } from "../state/session";
import { hush, sfx } from "./audio";
import { caseOf, Model } from "./Model";
import { Stage } from "./Stage";

interface Run {
  game: GameRow;
  /** Starts the game (true) or not (false, cancelled with Back). */
  go: (start: boolean) => void;
}

const store = createRoot(() => {
  const [run, setRun] = createSignal<Run | null>(null);
  // Gone once the game is running (KB is minimized by then) or the start failed.
  createEffect(() => session.running() && setRun(null));
  return { run, setRun };
});

/**
 * Plays the launch animation for a game (if it's on for this mode): its case
 * opens and the disc spins, or the cartridge goes into its console. Resolves
 * when it's time to start the emulator; false if the player went back.
 * Call `launchDone()` if the start then fails.
 */
export function launchAnimation(gameId: number): Promise<boolean> {
  const game = library.games().find((g) => g.id === gameId);
  if (!fx.launchFx() || !game || store.run()) return Promise.resolve(true);
  return new Promise((resolve) => store.setRun({ game, go: resolve }));
}

export const launchDone = () => store.setRun(null);

export function LaunchHost() {
  return (
    <Show when={store.run()} keyed>
      {(r) => <Launch run={r} />}
    </Show>
  );
}

function Launch(props: { run: Run }) {
  const g = props.run.game;
  const cart = caseOf(g.platformId).m === "cart";
  const [step, setStep] = createSignal<string[]>([]);
  const [settled, setSettled] = createSignal(false);
  const [msg, setMsg] = createSignal(false);
  const timers: ReturnType<typeof setTimeout>[] = [];
  const at = (ms: number, f: () => void) => timers.push(setTimeout(f, ms));
  const add = (c: string) => setStep((s) => [...s, c]);
  let started = false;
  onCleanup(() => timers.forEach(clearTimeout));

  sfx("select");
  at(60, () => setSettled(true));
  if (cart) {
    // The cartridge sound is timed so it seats when the cartridge does.
    // The cartridge comes out, the console slides in under it, and it goes in (a little longer than the first version, without a dead pause).
    at(520, () => {
      add("out");
      sfx("cart", 1410);
    });
    at(1500, () => add("drop"));
    at(2000, () => add("insert"));
    at(2430, () => {
      add("seated");
      setMsg(true);
    });
  } else {
    at(560, () => {
      add("open");
      sfx("disc");
    });
    at(1200, () => {
      add("spinning");
      setMsg(true);
    });
  }
  at(cart ? 3300 : 2400, () => {
    started = true;
    props.run.go(true);
  });

  // Back cancels while it plays; once the emulator is starting, input waits.
  onCleanup(
    pushHandler((a) => {
      if (a === "back" && !started) {
        hush();
        sfx("back");
        props.run.go(false);
        launchDone();
      }
      return true;
    }),
  );

  const platform = () => library.platforms().find((p) => p.id === g.platformId)?.name ?? "";
  return (
    <div class="launch-fx" classList={{ tv: fx.tv() }}>
      <Stage class="launch-stage">
        <div class="lc" classList={{ settled: settled(), cartlc: cart }}>
          <Model game={g} platformName={platform()} detail launch class={step().join(" ")} />
        </div>
        <div class="ltext">
          <b>{g.title}</b>
          <span class="lmsg" classList={{ on: msg() }}>
            {fx.tv() ? "Loading your game…" : "Starting…"}
          </span>
        </div>
      </Stage>
    </div>
  );
}
