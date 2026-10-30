import { createRoot, createSignal } from "solid-js";
import { api, errorText, events, type UpdateInfo, type UpdateProgress } from "../api";

/** The update being offered (dialog open), install progress and errors. */
function createUpdateStore() {
  const [offer, setOffer] = createSignal<UpdateInfo | null>(null);
  const [installing, setInstalling] = createSignal(false);
  const [progress, setProgress] = createSignal<UpdateProgress | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  /** Subscribes to the startup check's offer and download progress. Call once. */
  function listen() {
    void events.onUpdateAvailable((u) => {
      setError(null);
      setOffer(u);
    });
    void events.onUpdateProgress(setProgress);
  }

  /** "Check now": opens the dialog when there's an update; returns whether there was. */
  async function checkNow(): Promise<boolean> {
    setError(null);
    const u = await api.checkForUpdate();
    setOffer(u);
    return u !== null;
  }

  async function install() {
    setInstalling(true);
    setError(null);
    setProgress(null);
    try {
      // On success the app exits and the installer restarts it.
      await api.installUpdate();
    } catch (e) {
      setError(errorText(e));
      setInstalling(false);
    }
  }

  async function skip() {
    const u = offer();
    if (u) await api.setSetting("skipUpdateVersion", u.version).catch(() => {});
    setOffer(null);
  }

  return { offer, setOffer, installing, progress, error, listen, checkNow, install, skip };
}

export const updates = createRoot(createUpdateStore);
