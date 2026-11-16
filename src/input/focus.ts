// Spatial navigation: move DOM focus to the nearest focusable element in a
// direction, within a scope. Used by every screen except the game grid, which
// has its own index-based navigation.

export type Direction = "up" | "down" | "left" | "right";

const FOCUSABLE = [
  "button:not([disabled])",
  "input:not([disabled]):not([type=hidden])",
  "textarea:not([disabled])",
  "summary",
  "[tabindex]:not([tabindex='-1'])",
].join(",");

function visible(el: HTMLElement): boolean {
  if (el.getClientRects().length === 0) return false;
  const s = getComputedStyle(el);
  return s.visibility !== "hidden";
}

/** Focusable elements of `scope`, excluding ones inside nested scopes. */
export function focusables(scope: HTMLElement): HTMLElement[] {
  return Array.from(scope.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
    (el) => visible(el) && el.closest("[data-nav-scope]") === scope,
  );
}

export function focusEl(el: HTMLElement) {
  el.focus({ preventScroll: true });
  el.scrollIntoView({ block: "nearest", inline: "nearest" });
}

/** Focuses `[data-autofocus]` or the first focusable element. */
export function focusFirst(scope: HTMLElement): boolean {
  const list = focusables(scope);
  const target = list.find((el) => el.hasAttribute("data-autofocus")) ?? list[0];
  if (!target) return false;
  focusEl(target);
  return true;
}

/**
 * Picks the best candidate in `dir` from `from`: nearest along the direction,
 * strongly preferring elements that overlap on the other axis (same row or
 * column), so moving right stays on the current line when it can.
 */
export function pickNext(from: DOMRect, candidates: { rect: DOMRect }[], dir: Direction): number {
  const cx = from.left + from.width / 2;
  const cy = from.top + from.height / 2;
  let best = -1;
  let bestScore = Infinity;
  candidates.forEach(({ rect: r }, i) => {
    const rx = r.left + r.width / 2;
    const ry = r.top + r.height / 2;
    let primary: number;
    let orthogonal: number;
    let offAxis: number;
    switch (dir) {
      case "right":
        if (rx <= cx + 1) return;
        primary = Math.max(0, r.left - from.right);
        offAxis = Math.abs(ry - cy);
        orthogonal = Math.max(0, offAxis - (r.height + from.height) / 2);
        break;
      case "left":
        if (rx >= cx - 1) return;
        primary = Math.max(0, from.left - r.right);
        offAxis = Math.abs(ry - cy);
        orthogonal = Math.max(0, offAxis - (r.height + from.height) / 2);
        break;
      case "down":
        if (ry <= cy + 1) return;
        primary = Math.max(0, r.top - from.bottom);
        offAxis = Math.abs(rx - cx);
        orthogonal = Math.max(0, offAxis - (r.width + from.width) / 2);
        break;
      case "up":
        if (ry >= cy - 1) return;
        primary = Math.max(0, from.top - r.bottom);
        offAxis = Math.abs(rx - cx);
        orthogonal = Math.max(0, offAxis - (r.width + from.width) / 2);
        break;
    }
    // Same row/column beats closer-but-diagonal; offAxis breaks ties.
    const score = primary + orthogonal * 4 + offAxis * 0.05;
    if (score < bestScore) {
      bestScore = score;
      best = i;
    }
  });
  return best;
}

/** Moves focus within `scope`. Returns false when nothing lies that way. */
export function moveFocus(scope: HTMLElement, dir: Direction): boolean {
  const current = document.activeElement as HTMLElement | null;
  if (!current || !scope.contains(current) || current === scope) return focusFirst(scope);
  const list = focusables(scope).filter((el) => el !== current);
  const i = pickNext(
    current.getBoundingClientRect(),
    list.map((el) => ({ rect: el.getBoundingClientRect() })),
    dir,
  );
  if (i < 0) return false;
  focusEl(list[i]);
  return true;
}
