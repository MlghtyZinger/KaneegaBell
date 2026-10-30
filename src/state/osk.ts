import { createRoot, createSignal } from "solid-js";

export interface OskRequest {
  title: string;
  value: string;
  /** Called on every keystroke (live search). */
  onInput?: (value: string) => void;
  /** Extra line under the text, e.g. "12 matches". */
  status?: () => string;
  onDone: (value: string) => void;
  onCancel?: () => void;
}

export const osk = createRoot(() => {
  const [request, setRequest] = createSignal<OskRequest | null>(null);
  return {
    request,
    open: (r: OskRequest) => setRequest(r),
    close: () => setRequest(null),
  };
});

/**
 * Edits a real <input> with the on-screen keyboard: the field gets the result
 * and the same input/change events typing would fire, so existing handlers
 * (onInput / onChange) work unchanged.
 */
export function editWithOsk(input: HTMLInputElement | HTMLTextAreaElement) {
  const title = input.getAttribute("aria-label") ?? input.getAttribute("placeholder") ?? "Enter text";
  const setValue = (v: string, commit: boolean) => {
    input.value = v;
    input.dispatchEvent(new Event("input", { bubbles: true }));
    if (commit) input.dispatchEvent(new Event("change", { bubbles: true }));
  };
  osk.open({
    title,
    value: input.value,
    onInput: (v) => setValue(v, false),
    onDone: (v) => {
      setValue(v, true);
      input.focus({ preventScroll: true });
    },
    onCancel: () => input.focus({ preventScroll: true }),
  });
}
