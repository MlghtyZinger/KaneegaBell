import { createResource, For, Show } from "solid-js";
import { api } from "../api";
import { whatsNew } from "../state/whatsNew";
import { NavScope } from "./NavScope";
import { Hint } from "./Hints";

const LABELS: Record<string, string> = { new: "New", fixed: "Fixed", changed: "Changed", improved: "Improved" };

/** "2026-10-15" -> "15 Oct 2026" (anything else as written). */
function day(d: string | null): string {
  const m = d && /^(\d{4})-(\d{2})-(\d{2})$/.exec(d);
  if (!m) return d ?? "";
  return new Date(Date.UTC(+m[1], +m[2] - 1, +m[3])).toLocaleDateString(undefined, {
    day: "numeric",
    month: "short",
    year: "numeric",
    timeZone: "UTC",
  });
}

/** Every version's patch notes, newest first (from the README, built in). */
export function WhatsNew() {
  return (
    <Show when={whatsNew.open()}>
      <WhatsNewPanel />
    </Show>
  );
}

function WhatsNewPanel() {
  // Its own small chunk: loaded only when the window opens.
  const [notes] = createResource(() => import("virtual:patch-notes").then((m) => m.default));
  const [info] = createResource(() => api.appInfo().catch(() => null));
  const close = () => whatsNew.setOpen(false);

  return (
    <div class="overlay" onClick={(e) => e.target === e.currentTarget && close()}>
      <NavScope class="panel whats-new-panel" modal onBack={close}>
        <h2>What's new</h2>
        <p class="dim">Every KaneegaBell update and what it changed, newest first.</p>
        <div class="wn-list">
          <For each={notes()}>
            {(v, i) => (
              // Focusable, so the D-pad steps through the versions and the list scrolls.
              <section class="wn-version" tabindex="0" data-autofocus={i() === 0 ? "" : undefined}>
                <header class="wn-head">
                  <span class="wn-ver">{v.version}</span>
                  <Show when={v.version === info()?.version}>
                    <span class="wn-here">Your version</span>
                  </Show>
                  <span class="wn-date">{day(v.date)}</span>
                </header>
                <ul class="wn-changes">
                  <For each={v.changes}>
                    {(c) => (
                      <li>
                        <span class={`wn-tag wn-${c.kind || "note"}`}>{LABELS[c.kind] ?? "Note"}</span>
                        <span>{c.text}</span>
                      </li>
                    )}
                  </For>
                </ul>
              </section>
            )}
          </For>
        </div>
        <div class="panel-actions">
          <div class="hints-row">
            <Hint actions={["up", "down"]} label="Scroll" />
            <Hint action="back" label="Close" />
          </div>
          <button class="btn" onClick={close}>
            Close
          </button>
        </div>
      </NavScope>
    </div>
  );
}
