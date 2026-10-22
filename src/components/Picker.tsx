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
}) {
  const [openLocal, setOpenLocal] = createSignal(false);
  const open = () => props.open ?? openLocal();
  const setOpen = (v: boolean) => (props.onOpenChange ? props.onOpenChange(v) : setOpenLocal(v));
  const current = () => props.label ?? props.options.find((o) => o.value === props.value)?.label ?? props.value;

  const choose = (v: T) => {
    setOpen(false);
    if (v !== props.value) props.onChange(v);
  };

  return (
    <>
      <button class={`input picker ${props.class ?? ""}`} onClick={() => setOpen(true)} aria-haspopup="listbox">
        <span>{current()}</span>
        <span class="picker-caret" aria-hidden="true">
          ▾
        </span>
      </button>
      <Show when={open()}>
        {/* A portal: a transformed panel (the intro settings) would otherwise trap this fixed overlay inside its box. */}
        <Portal>
        <div class="overlay" onClick={(e) => e.target === e.currentTarget && setOpen(false)}>
          <NavScope class="panel picker-panel" modal onBack={() => setOpen(false)}>
            <Show when={props.title}>
              <h3>{props.title}</h3>
            </Show>
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
          </NavScope>
        </div>
        </Portal>
      </Show>
    </>
  );
}
