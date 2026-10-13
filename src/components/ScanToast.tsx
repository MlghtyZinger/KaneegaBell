import { createEffect, createSignal, on, onCleanup, Show } from "solid-js";
import { library } from "../state/library";
import { summarizeChanges } from "./ScanReport";

const SHOW_MS = 10_000;

/**
 * A short notice wherever you are when a scan changes the library ("Scan
 * finished: 3 added to PS2 · 1 removed"); the full list is in Settings.
 */
export function ScanToast(props: { hidden?: boolean }) {
  const [text, setText] = createSignal<string | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;
  const name = (id: string) => library.platforms().find((p) => p.id === id)?.shortName ?? id.toUpperCase();

  createEffect(
    on(
      library.report,
      (r) => {
        if (!r || r.changes.length === 0) return;
        setText(`${r.title}: ${summarizeChanges(r.changes, name)}`);
        clearTimeout(timer);
        timer = setTimeout(() => setText(null), SHOW_MS);
      },
      { defer: true },
    ),
  );
  onCleanup(() => clearTimeout(timer));

  return (
    <Show when={!props.hidden && text()}>
      <div class="toast" role="status" onClick={() => setText(null)}>
        {text()}
        <span class="dim"> · Full list in Settings → Library</span>
      </div>
    </Show>
  );
}
