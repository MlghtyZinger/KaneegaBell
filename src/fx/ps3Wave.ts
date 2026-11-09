/** Same color all day, independent of launch time and stable across desktop/TV. */
export function xmbHue(date: Date): number {
  const day = Math.floor(Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()) / 86_400_000);
  const colors = [275, 215, 185, 155, 85, 40, 18, 345, 310, 250, 205, 170];
  return colors[((day % colors.length) + colors.length) % colors.length];
}

/** Each ribbon bends independently, rather than rotating a fixed drawing. */
export function xmbWave(band: number, x: number, time: number): { y: number; width: number } {
  const phase = time * .42 + band * .19;
  return {
    y: .52 + Math.sin(x * 4.5 + phase) * .105
      + Math.sin(x * 7.3 - time * .27 + band * .31) * .026
      + (band - 3) * .009,
    width: .008 + (.5 + .5 * Math.sin(x * 5.2 + time * .31 + band * .65)) * .022,
  };
}
