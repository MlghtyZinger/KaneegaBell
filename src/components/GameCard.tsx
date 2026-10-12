import { createSignal, Show } from "solid-js";
import { artUrl, type GameRow } from "../api";

// Stable hue per title: the background of text-only cards, and the letterbox
// color around box art that doesn't match the card's shape.
function hue(s: string): number {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) | 0;
  return Math.abs(h) % 360;
}

export function GameCard(props: { game: () => GameRow; focused: () => boolean; platformLabel?: string }) {
  // Cards are recycled by the grid, so remember failures per game+version.
  const [failed, setFailed] = createSignal<string | null>(null);
  const artKey = () => `${props.game().id}:${props.game().art}`;
  const showArt = () => props.game().art > 0 && failed() !== artKey();

  return (
    <div
      class="card"
      classList={{ focused: props.focused(), "has-art": showArt() }}
      style={{ "--card-hue": hue(props.game().title) }}
    >
      <Show when={showArt()}>
        {/* Only the small WebP thumbnail is ever decoded in the grid. */}
        <img
          class="card-art"
          src={artUrl(props.game().id, props.game().art)}
          alt=""
          loading="lazy"
          decoding="async"
          draggable={false}
          onError={() => setFailed(artKey())}
        />
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
      <div class="card-caption">
        <div class="card-title">{props.game().title}</div>
        <Show when={props.game().variant}>
          <div class="card-variant">{props.game().variant}</div>
        </Show>
      </div>
    </div>
  );
}
