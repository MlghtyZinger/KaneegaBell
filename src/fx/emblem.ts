import master from "../../src-tauri/icons/icon-master.svg?raw";

/** The bull's tile colours per intro: KB's ember, Xbox green, GameCube indigo. */
const TILES = { ember: ["#d2462a", "#5a140a"], green: ["#6fd12a", "#0e4a08"], indigo: ["#7b6ae0", "#241a6a"] } as const;
const IDS = ["ember", "horn", "horns", "head", "ears", "tuft"];
let n = 0;

/** The app icon as inline SVG in another tile colour, with ids unique on the page. */
export function emblem(tile: keyof typeof TILES = "ember"): string {
  const k = n++;
  let s = master.replace(/<!--[\s\S]*?-->/g, "");
  s = s.replace('stop-color="#d2462a"', `stop-color="${TILES[tile][0]}"`).replace('stop-color="#5a140a"', `stop-color="${TILES[tile][1]}"`);
  for (const id of IDS) s = s.replaceAll(`id="${id}"`, `id="${id}${k}"`).replaceAll(`#${id})`, `#${id}${k})`).replaceAll(`href="#${id}"`, `href="#${id}${k}"`);
  return s.replace("<svg ", '<svg aria-hidden="true" ');
}
