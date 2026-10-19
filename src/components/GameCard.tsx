import { createSignal, Show } from "solid-js";
import { artUrl, type GameRow } from "../api";

// Stable hue per title: the background of text-only cards, and the letterbox
// color around box art that doesn't match the card's shape.
export function hue(s: string): number {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) | 0;
  return Math.abs(h) % 360;
}

export function GameCard(props: { game: () => GameRow; focused: () => boolean; platformLabel?: string; showIcon?: boolean }) {
  // Cards are recycled by the grid, so remember failures per game+version.
  const [failed, setFailed] = createSignal<string | null>(null);
  const artKey = () => `${props.game().id}:${props.game().art}`;
  const showArt = () => props.game().art > 0 && failed() !== artKey();
  // Known cover shape: the card takes it (see .card.fit).
  const aspect = () => {
    const a = props.game().artAspect;
    return showArt() && a ? Math.min(3, Math.max(0.3, a)) : null;
  };

  return (
    <div
      class="card"
      classList={{ focused: props.focused(), "has-art": showArt(), fit: aspect() !== null }}
      style={{ "--card-hue": hue(props.game().title), "--art-aspect": aspect() ?? undefined }}
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
      <Show when={props.showIcon && (props.game().icon ?? 0) > 0}>
        <img class="card-icon" src={artUrl(props.game().id, props.game().icon!, "i")} alt="" loading="lazy" decoding="async" />
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
