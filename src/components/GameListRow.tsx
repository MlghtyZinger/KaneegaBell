import { Show } from "solid-js";
import { artUrl, type GameRow } from "../api";
import { formatPlayTime } from "../state/session";
import { hue } from "./GameCard";

/** One row of the list view: icon (a coloured letter until it has one), title, platform, play time. */
export function GameListRow(props: { game: () => GameRow; focused: () => boolean; platformLabel?: string }) {
  const g = () => props.game();
  return (
    <div class="lrow" classList={{ focused: props.focused() }}>
      <Show
        when={(g().icon ?? 0) > 0}
        fallback={<div class="lrow-icon monogram" style={{ "--card-hue": hue(g().title) }}>{g().title.slice(0, 1).toUpperCase()}</div>}
      >
        <img class="lrow-icon" src={artUrl(g().id, g().icon!, "i")} alt="" loading="lazy" decoding="async" draggable={false} />
      </Show>
      <span class="lrow-title">
        {g().title}
        <Show when={g().variant}>
          <span class="lrow-variant"> {g().variant}</span>
        </Show>
      </span>
      <Show when={g().favorite}>
        <span class="lrow-fav" aria-label="Favorite">★</span>
      </Show>
      <Show when={props.platformLabel}>
        <span class="lrow-meta">{props.platformLabel}</span>
      </Show>
      <span class="lrow-time">{g().playSeconds > 0 ? formatPlayTime(g().playSeconds) : ""}</span>
    </div>
  );
}
