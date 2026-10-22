/** Dead zone and speed curve for the Wii's screen-local right-stick pointer. */
export function pointerDelta(axis: number, elapsedMs: number, width: number): number {
  if (!Number.isFinite(axis) || !Number.isFinite(elapsedMs) || !Number.isFinite(width) || width <= 0) return 0;
  const amount = Math.max(0, (Math.min(1, Math.abs(axis)) - .2) / .8);
  if (amount === 0) return 0;
  return Math.sign(axis) * amount * amount * Math.min(40, Math.max(0, elapsedMs)) * width * .00065;
}
