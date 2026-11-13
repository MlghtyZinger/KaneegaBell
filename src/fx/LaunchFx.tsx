import { isLauncher, launcherName } from "../state/launchers";
import { createRoot, createSignal, onCleanup, Show } from "solid-js";
import { api, events, type GameRow } from "../api";
import { pushHandler } from "../input/actions";
import { fx } from "../state/fx";
import { library } from "../state/library";
import { hush, sfx } from "./audio";
import { caseOf, Model } from "./Model";
import { Stage } from "./Stage";
import { NavScope } from "../components/NavScope";

interface Run {
  game: GameRow;
  /** Finishes or skips the presentation; the game is already starting. */
  go: (start: boolean) => void;
}

const store = createRoot(() => {
  const [run, setRun] = createSignal<Run | null>(null);
  return { run, setRun };
});

/** The existing case/cartridge timeline, now concurrent with emulator startup. */
function launchAnimation(gameId: number): Promise<boolean> {
  const game = library.games().find((g) => g.id === gameId);
  if (!fx.launchFx() || !game || store.run()) return Promise.resolve(true);
  return new Promise((resolve) => store.setRun({ game, go: resolve }));
}

export const launchDone = () => store.setRun(null);
const [steamWaiting, setSteamWaiting] = createRoot(() => createSignal<GameRow | null>(null));

const [presenting, setPresenting] = createRoot(() => createSignal(false));
export const launchPresenting = presenting;

/** Spawn immediately, keeping the presentation visible until completion or Back. */
export async function playGame(gameId: number, ps2Duration?: number) {
  if (presenting()) return;
  setPresenting(true);
  document.documentElement.classList.add("launch-presenting");
  const game = library.games().find((g) => g.id === gameId);
  if (game && !isLauncher(game.platformId) && (!fx.launchFx() || ps2Duration !== undefined)) sfx(caseOf(game.platformId).m === "cart" ? "cart" : "disc");
  let timer: ReturnType<typeof setTimeout> | undefined;
  let skip = () => {};
  const steam = !!game && isLauncher(game.platformId);
  const animation = steam ? Promise.resolve(true) : ps2Duration === undefined ? launchAnimation(gameId) : new Promise<boolean>((resolve) => {
    skip = () => resolve(true);
    timer = setTimeout(skip, ps2Duration);
  });
  const remove = pushHandler((a) => {
    if (a === "back") { store.run()?.go(true); launchDone(); skip(); }
    return !steam; // The waiting panel owns its cancel control.
  }, true);
  let stopWaitingListener: (() => void) | undefined;
  try {
    if (steam) stopWaitingListener = await events.onClientLaunchWaiting((id) => { if (id === gameId && game) setSteamWaiting(game); });
    await Promise.all([api.launchGame(gameId, true), animation]);
    await api.finishLaunch(gameId);
  } finally {
    stopWaitingListener?.();
    setSteamWaiting(null);
    clearTimeout(timer);
    remove();
    launchDone();
    document.documentElement.classList.remove("launch-presenting");
    setPresenting(false);
  }
}

export function LaunchHost() {
  return (
    <>
      <Show when={steamWaiting()}>{(g) => <div class="overlay"><NavScope class="panel" modal onBack={() => void api.cancelClientLaunch(g().id)}>
        <p class="dim">Waiting for {launcherName(g().platformId)}</p><h2>{g().title}</h2>
        <p>Play time starts when the game is detected. The client may show its normal login, update or launch options.</p>
        <button class="btn" data-autofocus onClick={() => void api.cancelClientLaunch(g().id)}>Cancel wait</button>
        <p class="dim small">Cancelling stops KB's wait; The client or game may still open.</p>
      </NavScope></div>}</Show>
      <Show when={store.run()} keyed>{(r) => <Launch run={r} />}</Show>
    </>
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
    props.run.go(true);
    launchDone();
  });

  // Back cancels while it plays; once the emulator is starting, input waits.
  onCleanup(
    pushHandler((a) => {
      if (a === "back") {
        hush();
        sfx("back");
        props.run.go(true);
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
