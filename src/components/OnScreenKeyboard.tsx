import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { setKeyInterceptor } from "../input/actions";
import { osk, type OskRequest } from "../state/osk";
import { NavScope } from "./NavScope";
import { Hint } from "./Hints";

const ROWS = [
  ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"],
  ["q", "w", "e", "r", "t", "y", "u", "i", "o", "p"],
  ["a", "s", "d", "f", "g", "h", "j", "k", "l", "'"],
  ["z", "x", "c", "v", "b", "n", "m", ",", ".", "-"],
];
const SYMBOLS = [
  ["!", "@", "#", "$", "%", "&", "(", ")", "[", "]"],
  ["+", "=", "_", ":", ";", "~", "\\", "/", "?", "\""],
];

/** Renders the active on-screen keyboard request, if any. Mounted once at the app root. */
export function OskHost() {
  // Keyed: the keyboard gets the request object itself. Done/Cancel close the
  // keyboard and then call its callbacks; a non-keyed accessor would throw a
  // stale-read error at that point.
  return (
    <Show when={osk.request()} keyed>
      {(r) => <OnScreenKeyboard req={r} />}
    </Show>
  );
}

function OnScreenKeyboard(props: { req: OskRequest }) {
  const [value, setValue] = createSignal(props.req.value);
  const [shift, setShift] = createSignal(false);
  const [symbols, setSymbols] = createSignal(false);

  const update = (v: string) => {
    setValue(v);
    props.req.onInput?.(v);
  };
  const type = (s: string) => {
    update(value() + (shift() ? s.toUpperCase() : s));
    if (shift()) setShift(false); // one-shot, like a phone keyboard
  };
  const backspace = () => update(value().slice(0, -1));
  const done = () => {
    osk.close();
    props.req.onDone(value());
  };
  const cancel = () => {
    osk.close();
    props.req.onInput?.(props.req.value); // undo live edits
    props.req.onCancel?.();
  };

  // A physical keyboard types straight into the on-screen one.
  onMount(() => {
    setKeyInterceptor((e) => {
      if (e.ctrlKey) {
        // Ctrl+V is the window's paste event below; copy and cut write the clipboard here.
        const k = e.key.toLowerCase();
        if (k !== "c" && k !== "x") return false;
        void navigator.clipboard.writeText(value()).catch(() => {});
        if (k === "x") update("");
        return true;
      }
      if (e.key === "Enter") done();
      else if (e.key === "Escape") cancel();
      else if (e.key === "Backspace") backspace();
      else if (e.key.length === 1) update(value() + e.key);
      else return false; // arrows etc. still navigate the keys
      return true;
    });
  });
  const onPaste = (e: ClipboardEvent) => {
    e.preventDefault();
    const t = e.clipboardData?.getData("text/plain").replace(/\s+/g, " ");
    if (t) update(value() + t);
  };
  window.addEventListener("paste", onPaste);
  onCleanup(() => {
    setKeyInterceptor(null);
    window.removeEventListener("paste", onPaste);
  });

  const rows = () => (symbols() ? [ROWS[0], ...SYMBOLS, ROWS[3]] : ROWS);

  return (
    <div class="overlay osk-overlay">
      <NavScope
        class="panel osk"
        modal
        onBack={() => (value() ? backspace() : cancel())}
        onSecondary={() => type(" ")}
        onMenu={done}
        onFavorite={() => setShift((s) => !s)}
      >
        <div class="osk-title dim">{props.req.title}</div>
        <div class="osk-value mono" aria-live="polite">
          {value()}
          <span class="osk-caret" />
        </div>
        <Show when={props.req.status}>{(status) => <div class="osk-status dim">{status()()}</div>}</Show>
        <div class="osk-keys">
          <For each={rows()}>
            {(row, r) => (
              <div class="osk-row">
                <For each={row}>
                  {(k, c) => (
                    <button
                      class="osk-key"
                      data-autofocus={r() === 1 && c() === 0 ? "" : undefined}
                      onClick={() => type(k)}
                    >
                      {shift() ? k.toUpperCase() : k}
                    </button>
                  )}
                </For>
              </div>
            )}
          </For>
          <div class="osk-row">
            <button class="osk-key wide" classList={{ on: shift() }} onClick={() => setShift((s) => !s)}>
              ⇧ Shift
            </button>
            <button class="osk-key wide" classList={{ on: symbols() }} onClick={() => setSymbols((s) => !s)}>
              #+=
            </button>
            <button class="osk-key space" onClick={() => type(" ")}>
              Space
            </button>
            <button class="osk-key wide" onClick={backspace}>
              ⌫
            </button>
            <button class="osk-key wide primary" onClick={done}>
              Done
            </button>
          </div>
        </div>
        <div class="hints-row">
          <Hint action="select" label="Type" />
          <Hint action="back" label="Delete" />
          <Hint action="secondary" label="Space" />
          <Hint action="favorite" label="Shift" />
          <Hint action="menu" label="Done" />
        </div>
      </NavScope>
    </div>
  );
}
