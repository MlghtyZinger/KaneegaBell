import { Show } from "solid-js";
import { updates } from "../state/updates";
import { session } from "../state/session";
import { NavScope } from "./NavScope";
import { Hint } from "./Hints";

const mb = (n: number) => `${(n / 1048576).toFixed(1)} MB`;

/** "A new version is available": changelog, Update now / Later / Skip this version. */
export function UpdateDialog() {
  const close = () => !updates.installing() && updates.setOffer(null);
  const pct = () => {
    const p = updates.progress();
    return p?.total ? Math.min(100, Math.round((p.downloaded / p.total) * 100)) : null;
  };

  return (
    <Show when={updates.offer()}>
      {(u) => (
        <div class="overlay" onClick={(e) => e.target === e.currentTarget && close()}>
          <NavScope class="panel update-panel" modal onBack={close}>
            <h2>KaneegaBell {u().version} is available</h2>
            <p class="dim">You have {u().currentVersion}.</p>
            <Show when={u().notes}>
              <div class="update-notes">{u().notes}</div>
            </Show>
            <Show when={updates.error()}>
              <p class="warn">{updates.error()}</p>
            </Show>
            <Show
              when={updates.installing()}
              fallback={
                <div class="row">
                  <button
                    class="btn primary"
                    data-autofocus
                    disabled={!!session.running()}
                    onClick={() => void updates.install()}
                  >
                    Update now
                  </button>
                  <button class="btn" onClick={() => updates.setOffer(null)}>
                    Later
                  </button>
                  <button class="btn" onClick={() => void updates.skip()}>
                    Skip this version
                  </button>
                  <Show when={session.running()}>
                    <span class="dim small">Available after you close the game.</span>
                  </Show>
                </div>
              }
            >
              <div class="update-progress">
                <div class="ach-bar">
                  <div style={{ width: `${pct() ?? 0}%` }} />
                </div>
                <p class="dim">
                  {updates.progress()
                    ? pct() === 100
                      ? "Installing… KaneegaBell will close and start again by itself."
                      : `Downloading… ${mb(updates.progress()!.downloaded)}${updates.progress()!.total ? ` of ${mb(updates.progress()!.total!)}` : ""}`
                    : "Starting the download…"}
                </p>
              </div>
            </Show>
            <div class="panel-actions">
              <div class="hints-row">
                <Hint action="select" label="Choose" />
                <Hint action="back" label="Later" />
              </div>
            </div>
          </NavScope>
        </div>
      )}
    </Show>
  );
}
