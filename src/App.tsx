import { onMount } from "solid-js";
import { getCurrentWindow } from "@tauri-apps/api/window";

export default function App() {
  // The window starts hidden (tauri.conf.json) so there's no white flash;
  // show it once the first frame is ready.
  onMount(() => requestAnimationFrame(() => void getCurrentWindow().show()));

  return (
    <main class="app">
      <h1>Emu Frontend</h1>
    </main>
  );
}
