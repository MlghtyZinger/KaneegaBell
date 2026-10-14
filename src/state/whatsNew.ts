import { createSignal } from "solid-js";
import { api } from "../api";

/** The "What's new" window: every version's patch notes. */
function createWhatsNew() {
  const [open, setOpen] = createSignal(false);

  /**
   * At startup: opens once after KB was updated (the version differs from the
   * last one seen). Not on a first start, and not in build-folder copies.
   */
  async function showIfUpdated() {
    const info = await api.appInfo();
    if (info.devBuild) return;
    const seen = await api.getSetting<string>("lastSeenVersion");
    if (seen === info.version) return;
    await api.setSetting("lastSeenVersion", info.version);
    if (seen) setOpen(true);
  }

  return { open, setOpen, showIfUpdated };
}

export const whatsNew = createWhatsNew();
