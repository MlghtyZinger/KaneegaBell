import { createResource, createSignal, Show } from "solid-js";
import { api, type ManualStatus } from "../api";
import { manual } from "../state/manual";

/** Why the button is greyed out, or what pressing it does. */
export function manualHint(s: ManualStatus | undefined): string {
  switch (s?.state) {
    case "off": return "Manual downloads are off. Turn them on in Settings → Game info, or pick a file of your own in Artwork → Manual.";
    case "none": return "No manual found for this game. Search for one, or pick a file of your own, in Artwork → Manual.";
    case "available": return `Downloads the manual when opened (${s.source ?? "Internet Archive"}).`;
    case "unknown": return "Looks for the manual when opened.";
    default: return s?.source ? `From ${s.source}` : "Opens the game's manual.";
  }
}

/**
 * The Manual button: always shown; greyed out (but still reachable, so it can say why) when downloads
 * are off and there's no file of the user's own, or when nothing was found. Reads only what's known
 * already: finding and downloading happen when it's pressed.
 */
export function ManualButton(props: { gameId: number; title: string; class?: string }) {
  const [status] = createResource(() => props.gameId, (id) => api.manualStatus(id).catch(() => undefined));
  const [why, setWhy] = createSignal(false);
  const dim = () => status()?.state === "off" || status()?.state === "none";
  return (
    <>
      <button
        class={props.class ?? "btn"}
        classList={{ "manual-off": dim() }}
        aria-disabled={dim() ? "true" : undefined}
        title={manualHint(status())}
        onClick={() => (dim() ? setWhy(!why()) : manual.show(props.gameId, props.title))}
        onBlur={() => setWhy(false)}
      >
        {status()?.state === "off" ? "Manual (downloads off)" : status()?.state === "none" ? "Manual (none found)" : "Manual"}
      </button>
      <Show when={why()}>
        <p class="manual-why dim small">{manualHint(status())}</p>
      </Show>
    </>
  );
}
