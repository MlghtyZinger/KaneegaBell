import { createRoot, createSignal } from "solid-js";

/** The game whose manual is open (anywhere in KB), or null. `overlay`: opened over a running game
 *  (`backdrop`: a picture of the game was taken to show blurred behind it). */
export interface OpenManual {
  gameId: number;
  title: string;
  overlay: boolean;
  backdrop: boolean;
}

export const manual = createRoot(() => {
  const [open, setOpen] = createSignal<OpenManual | null>(null);
  // Where each manual was left (its spread), so it opens there again while KB runs.
  const places = new Map<number, number>();
  return {
    open,
    show: (gameId: number, title: string, overlay = false, backdrop = false) => setOpen({ gameId, title, overlay, backdrop }),
    close: () => setOpen(null),
    place: (gameId: number) => places.get(gameId) ?? 0,
    remember: (gameId: number, spread: number) => places.set(gameId, spread),
  };
});
