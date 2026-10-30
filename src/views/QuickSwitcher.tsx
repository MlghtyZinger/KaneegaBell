import { For, Show } from "solid-js";
import { Icon, PlatformIcon } from "../components/Icons";
import { Hint } from "../components/Hints";
import { NavScope } from "../components/NavScope";
import { ManualButton } from "../components/ManualButton";

const LIB = ["favorites", "recent", "collections"];

/** A library tab's icon: Favorites, Recent, Collections and All have their own; platforms use their picture. */
export function TabIcon(props: { id: string }) {
  return (
    <Show when={{ favorites: "star", recent: "clock", collections: "stack", all: "all" }[props.id] as "star" | undefined} fallback={<PlatformIcon id={props.id} />}>
      {(name) => <Icon name={name()} />}
    </Show>
  );
}

/** △ in TV Mode: every library tab at once, drawn in the current menu style. PS3 also lists the focused game's options, as its own △ menu does. */
export function QuickSwitcher(props: {
  style: "kb" | "wii" | "ps3" | "ps2";
  tabs: string[];
  current: string;
  label: (id: string) => string;
  count: (id: string) => string;
  onPick: (id: string) => void;
  onClose: () => void;
  game?: { title: string; play: () => void; info: () => void; cover: () => void; id: number };
}) {
  const lib = () => props.tabs.filter((id) => LIB.includes(id));
  const rest = () => props.tabs.filter((id) => !LIB.includes(id));
  // PS3 style lists the game's options first: start there, at the top of the panel.
  const gameFirst = () => props.style === "ps3" && !!props.game;
  const Item = (p: { id: string; big?: boolean }) => (
    <button
      class="qs-item"
      classList={{ "qs-big": p.big, active: props.current === p.id }}
      data-autofocus={props.current === p.id && !gameFirst() ? "" : undefined}
      onClick={() => props.onPick(p.id)}
    >
      <TabIcon id={p.id} />
      <span><strong>{props.label(p.id)}</strong><small>{props.count(p.id)}</small></span>
    </button>
  );
  return (
    <div class={`overlay qs-overlay qs-${props.style}`} onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <NavScope class="qs" modal onBack={props.onClose} onFavorite={props.onClose}>
        <Show when={props.style === "ps3" && props.game}>
          {(g) => (
            <div class="qs-game">
              <p>{g().title}</p>
              <button class="qs-item" data-autofocus onClick={g().play}>Start</button>
              <button class="qs-item" onClick={g().info}>Information</button>
              <button class="qs-item" onClick={g().cover}>View cover</button>
              <ManualButton gameId={g().id} title={g().title} class="qs-item" />
            </div>
          )}
        </Show>
        <Show when={props.style !== "ps3"}><h2>{props.style === "wii" ? "Choose a channel group" : "Go to"}</h2></Show>
        <div class="qs-lib"><For each={lib()}>{(id) => <Item id={id} big />}</For></div>
        <Show when={props.style === "kb"}><p class="qs-head">Platforms</p></Show>
        <div class="qs-plat"><For each={rest()}>{(id) => <Item id={id} />}</For></div>
        <footer class="qs-hints"><Hint action="select" label="Go" /><Hint action="back" label="Close" /></footer>
      </NavScope>
    </div>
  );
}
