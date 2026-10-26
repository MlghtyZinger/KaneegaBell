import { createSignal, For, Show } from "solid-js";
import { Portal } from "solid-js/web";
import { NavScope } from "./NavScope";

export interface PickerOption<T extends string> {
  value: T;
  label: string;
}

/**
 * A controller-friendly replacement for <select>: a button showing the current
 * choice that opens a focusable list. (Native dropdowns can't be driven by a
 * gamepad.)
 */
export function Picker<T extends string>(props: {
  value: T;
  options: PickerOption<T>[];
  onChange: (v: T) => void;
  title?: string;
  class?: string;
  /** Optional outside control of the list (e.g. opened by a controller button). */
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  /** Button text instead of the current option's label. */
  label?: string;
  /** Open as a menu right under the button instead of a panel in the middle of the screen. */
  dropdown?: boolean;
}) {
  const [openLocal, setOpenLocal] = createSignal(false);
  const open = () => props.open ?? openLocal();
  const setOpen = (v: boolean) => (props.onOpenChange ? props.onOpenChange(v) : setOpenLocal(v));
  const current = () => props.label ?? props.options.find((o) => o.value === props.value)?.label ?? props.value;

  let button!: HTMLButtonElement;
  // Where the dropdown goes: under the button (or over it when there's no room), right edges aligned.
  const [menu, setMenu] = createSignal<{ right: number; top?: number; bottom?: number; minWidth: number; maxHeight: number } | null>(null);
  const openPicker = () => {
    if (props.dropdown) {
      const r = button.getBoundingClientRect();
      const below = window.innerHeight - r.bottom - 12;
      const above = r.top - 12;
      const flip = below < 220 && above > below;
      setMenu({
        right: Math.max(8, window.innerWidth - r.right),
        minWidth: r.width,
        maxHeight: Math.min(360, flip ? above : below),
        ...(flip ? { bottom: window.innerHeight - r.top + 4 } : { top: r.bottom + 4 }),
      });
    } else {
      setMenu(null);
    }
    setOpen(true);
  };

  const choose = (v: T) => {
    setOpen(false);
    if (v !== props.value) props.onChange(v);
  };

  const Options = () => (
    <div class="picker-list" role="listbox">
      <For each={props.options}>
        {(o) => (
          <button
            class="picker-option"
            classList={{ selected: o.value === props.value }}
            role="option"
            aria-selected={o.value === props.value}
            data-autofocus={o.value === props.value ? "" : undefined}
            onClick={() => choose(o.value)}
          >
            {o.label}
          </button>
        )}
      </For>
    </div>
  );

  return (
    <>
      <button ref={button} class={`input picker ${props.class ?? ""}`} onClick={openPicker} aria-haspopup="listbox" aria-expanded={open()}>
        <span>{current()}</span>
        <span class="picker-caret" aria-hidden="true">
          ▾
        </span>
      </button>
      <Show when={open()}>
        {/* A portal: a transformed panel (the intro settings) would otherwise trap this fixed overlay inside its box. */}
        <Portal>
          <Show
            when={menu()}
            fallback={
              <div class="overlay" onClick={(e) => e.target === e.currentTarget && setOpen(false)}>
                <NavScope class="panel picker-panel" modal onBack={() => setOpen(false)}>
                  <Show when={props.title}>
                    <h3>{props.title}</h3>
                  </Show>
                  <Options />
                </NavScope>
              </div>
            }
          >
            {(m) => (
              <div class="menu-scrim" onClick={(e) => e.target === e.currentTarget && setOpen(false)}>
                <div
                  class="picker-menu-wrap"
                  style={{
                    right: `${m().right}px`,
                    top: m().top !== undefined ? `${m().top}px` : undefined,
                    bottom: m().bottom !== undefined ? `${m().bottom}px` : undefined,
                    "min-width": `${m().minWidth}px`,
                    "max-height": `${m().maxHeight}px`,
                  }}
                >
                  <NavScope class="picker-menu" modal onBack={() => setOpen(false)}>
                    <Options />
                  </NavScope>
                </div>
              </div>
            )}
          </Show>
        </Portal>
      </Show>
    </>
  );
}
