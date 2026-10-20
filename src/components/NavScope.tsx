import { onCleanup, onMount, type JSX } from "solid-js";
import { device, isTextField, pushHandler, type Action } from "../input/actions";
import { focusFirst, moveFocus } from "../input/focus";
import { editWithOsk } from "../state/osk";

interface Props {
  children: JSX.Element;
  class?: string;
  /** B / Esc. When omitted, "back" falls through to scopes below. */
  onBack?: () => void;
  /** L1/R1 (or Q/E): switch tabs/sections. */
  onTab?: (delta: 1 | -1) => void;
  onSecondary?: () => void;
  onMenu?: () => void;
  onFavorite?: () => void;
  /** Modal scopes swallow every action, even unhandled ones. */
  modal?: boolean;
  /** Focus the first element (or [data-autofocus]) on mount. Default true. */
  autofocus?: boolean;
  ref?: (el: HTMLDivElement) => void;
}

/** Presses the focused control the way A / Enter should. */
export function activate(el: Element | null) {
  if (!(el instanceof HTMLElement)) return;
  if (isTextField(el)) {
    // Controller: type with the on-screen keyboard. Keyboard: the field already
    // has focus, so typing just works.
    if (device() === "gamepad") editWithOsk(el);
    else el.focus();
    return;
  }
  el.click(); // buttons, checkboxes (toggles + fires change), summary, links
}

/**
 * A region navigated with D-pad/arrows by spatial focus. Scopes stack: the
 * most recently mounted one receives actions first, so overlays naturally
 * take over from the screen beneath them.
 */
export function NavScope(props: Props) {
  let el!: HTMLDivElement;
  const previous = document.activeElement as HTMLElement | null;
  // Until the user moves, late-rendering content (e.g. a Play button that
  // appears once game data loads) may claim initial focus via [data-autofocus].
  let userMoved = false;

  const handler = (a: Action): boolean => {
    userMoved = true;
    switch (a) {
      case "up":
      case "down":
      case "left":
      case "right":
        // Nothing focusable that way: scroll the scope, so text and boxes without
        // a button (description, achievements) can still be read with a pad.
        if (!moveFocus(el, a) && (a === "up" || a === "down")) {
          el.scrollBy({ top: (a === "down" ? 1 : -1) * el.clientHeight * 0.8 });
        }
        return true;
      case "select": {
        const active = document.activeElement;
        if (active && el.contains(active) && active !== el) activate(active);
        else focusFirst(el);
        return true;
      }
      case "back":
        if (props.onBack) {
          props.onBack();
          return true;
        }
        break;
      case "prevPlatform":
      case "nextPlatform":
        if (props.onTab) {
          props.onTab(a === "prevPlatform" ? -1 : 1);
          return true;
        }
        break;
      case "secondary":
        if (props.onSecondary) {
          props.onSecondary();
          return true;
        }
        break;
      case "menu":
        if (props.onMenu) {
          props.onMenu();
          return true;
        }
        break;
      case "favorite":
        if (props.onFavorite) {
          props.onFavorite();
          return true;
        }
        break;
    }
    return !!props.modal;
  };

  onMount(() => {
    onCleanup(pushHandler(handler));
    if (props.autofocus === false) return;
    queueMicrotask(() => focusFirst(el));
    const observer = new MutationObserver(() => {
      if (userMoved) return observer.disconnect();
      const preferred = el.querySelector<HTMLElement>("[data-autofocus]");
      if (preferred && document.activeElement !== preferred && preferred.closest("[data-nav-scope]") === el) {
        focusFirst(el);
        observer.disconnect();
      }
    });
    observer.observe(el, { childList: true, subtree: true });
    onCleanup(() => observer.disconnect());
  });
  onCleanup(() => {
    // Give focus back to whatever had it before (e.g. the button that opened an overlay).
    if (previous && previous.isConnected) previous.focus({ preventScroll: true });
  });

  return (
    <div
      class={props.class}
      data-nav-scope
      ref={(e) => {
        el = e;
        props.ref?.(e);
      }}
    >
      {props.children}
    </div>
  );
}
