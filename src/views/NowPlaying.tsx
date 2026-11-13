import { createResource, createSignal, For, onCleanup } from "solid-js";
import { api, type ExitCombo, type RunningGame } from "../api";
import { pushHandler } from "../input/actions";
import { buttonLabel, session } from "../state/session";

/**
 * Shown (in a minimized window) while a game runs. Deliberately static:
 * no animation, nothing to render, so the frontend costs ~nothing.
 */
export function NowPlaying(props: { game: RunningGame }) {
  const [combo] = createResource(async () => {
    const s = await api.getSettings();
    return (s.values.exitCombo as ExitCombo | undefined) ?? { buttons: ["back", "start"], holdMs: 1000 };
  });
  const [closing, setClosing] = createSignal(false);

  // Swallow navigation while a game runs.
  onCleanup(pushHandler(() => true));

  const close = () => {
    setClosing(true);
    void api.exitGame();
  };

  return (
    <div class="now-playing">
      <p class="dim">Now playing</p>
      <h1>{props.game.title}</h1>
      <p class="dim">
        To quit, hold{" "}
        <For each={combo()?.buttons ?? []}>
          {(b, i) => (
            <>
              {i() > 0 && " + "}
              <kbd>{buttonLabel(b, session.lastPad()?.family)}</kbd>
            </>
          )}
        </For>{" "}
        for {((combo()?.holdMs ?? 1000) / 1000).toFixed(1)} s, or close the game in the emulator.
      </p>
      <button class="btn" disabled={closing()} onClick={close}>
        {closing() ? "Closing…" : "Close game"}
      </button>
    </div>
  );
}
