import { Show } from "solid-js";
import type { GameRow } from "../api";

// Stable hue per title so text-only cards are easy to tell apart before artwork
// exists (milestone d replaces the background with a WebP thumbnail).
function hue(s: string): number {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) | 0;
  return Math.abs(h) % 360;
}

export function GameCard(props: { game: () => GameRow; focused: () => boolean; platformLabel?: string }) {
  return (
    <div
      class="card"
      classList={{ focused: props.focused() }}
      style={{ "--card-hue": hue(props.game().title) }}
    >
      <div class="card-title">{props.game().title}</div>
      <Show when={props.game().variant}>
        <div class="card-variant">{props.game().variant}</div>
      </Show>
      <div class="card-badges">
        <Show when={props.platformLabel}>
          <span class="badge">{props.platformLabel}</span>
        </Show>
        <Show when={props.game().isMultidisc}>
          <span class="badge">Multi-disc</span>
        </Show>
        <Show when={props.game().favorite}>
          <span class="badge fav">★</span>
        </Show>
      </div>
    </div>
  );
}
