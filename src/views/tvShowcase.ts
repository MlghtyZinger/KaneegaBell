/** A bounded logo list, keeping the selected item visible even at either end. */
export function showcaseWindow(focused: number, count: number, slots = 5) {
  const size = Math.min(Math.max(0, count), Math.max(1, slots));
  const start = Math.min(Math.max(0, focused - Math.floor(size / 2)), Math.max(0, count - size));
  return Array.from({ length: size }, (_, i) => start + i);
}

/** Wii channel pages stay bounded even for a library with thousands of games. */
export function channelWindow(focused: number, count: number) {
  const start = Math.floor(Math.max(0, focused) / 12) * 12;
  return Array.from({ length: Math.max(0, Math.min(12, count - start)) }, (_, i) => start + i);
}

/** Fit the full case, including unusually wide or tall cover art, in its display area. */
export function showcaseScale(width: number, height: number, boxWidth: number, boxHeight: number, aspect: number | null) {
  const ratio = aspect ? Math.min(2.5, Math.max(0.4, aspect)) : boxHeight / boxWidth;
  const h = Math.sqrt(boxWidth * boxHeight * ratio);
  return Math.max(0.1, Math.min(width * 0.8 / (h / ratio), height * 0.8 / h));
}

/** One previous item above the crossbar and four compact following items around a fixed XMB selection position. */
export function xmbWindow(focused: number, count: number) {
  const start = Math.max(0, focused - 1);
  return Array.from({ length: Math.max(0, Math.min(count - start, focused + 5 - start)) }, (_, i) => start + i);
}
