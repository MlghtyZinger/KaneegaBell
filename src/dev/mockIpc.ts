// Dev-only: lets the UI run in a plain browser (no Tauri) with fake data, for
// layout work and grid stress tests. Loaded via a dynamic import guarded by
// import.meta.env.DEV, so it never ships in release builds.
// Add ?games=N to the URL to change the library size (default 5000), and
// ?art=1 to give games cover shapes (for card layout work).
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import type { GameRow, Library, Settings } from "../api";

const PLATFORMS = [
  ["snes", "Super Nintendo", "SNES"],
  ["n64", "Nintendo 64", "N64"],
  ["gba", "Game Boy Advance", "GBA"],
  ["gc", "GameCube", "GC"],
  ["nds", "Nintendo DS", "NDS"],
  ["wii", "Wii", "Wii"],
  ["3ds", "Nintendo 3DS", "3DS"],
  ["segacd", "Sega CD", "Sega CD"],
  ["ps1", "PlayStation", "PS1"],
  ["ps2", "PlayStation 2", "PS2"],
  ["psp", "PlayStation Portable", "PSP"],
  ["ps3", "PlayStation 3", "PS3"],
] as const;

const WORDS =
  "Shadow Legend Dragon Quest Star Fire Emblem Kingdom Hearts Metal Gear Solid Final Fantasy Castle Night Moon Sun Dark Dawn Golden Silver Crystal Ocean Sky Iron Storm Thunder Blade Saga Chronicles Tales Mystic Ancient City Lost Age Revenge Return Rise Fall Empire Galaxy Super Mario Kart Racing Ninja Samurai Spirit Ghost Soul Heart Mind Hero Knight"
    .split(" ");

// mulberry32: tiny seeded PRNG with decent distribution.
function rng(seed: number) {
  return () => {
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// Typical cover shapes (height / width), for ?art=1 layout tests.
const ASPECTS: Record<string, number> = {
  snes: 0.72, n64: 0.73, gba: 1, gc: 1.4, nds: 0.9, wii: 1.4, "3ds": 0.88,
  segacd: 1, ps1: 1, ps2: 1.4, psp: 1.72, ps3: 1.16,
};

function makeLibrary(n: number): Library {
  const withArt = new URLSearchParams(location.search).get("art") === "1";
  const r = rng(42);
  const pick = () => WORDS[Math.floor(r() * WORDS.length)];
  const games: GameRow[] = [];
  for (let i = 0; i < n; i++) {
    const words = 1 + Math.floor(r() * 4);
    let title = Array.from({ length: words }, pick).join(" ");
    if (r() < 0.3) title += `: ${pick()} ${pick()}`;
    if (r() < 0.15) title += ` ${2 + Math.floor(r() * 4)}`;
    const platformId = PLATFORMS[Math.floor(r() * PLATFORMS.length)][0];
    games.push({
      id: i + 1,
      platformId,
      title,
      variant: r() < 0.05 ? "T-En v1.2" : null,
      region: "USA",
      favorite: r() < 0.03,
      playSeconds: 0,
      lastPlayed: null,
      isMultidisc: r() < 0.03,
      // No images exist in the mock: cards fall back to text, but keep the cover's shape.
      art: withArt ? 1 : 0,
      artAspect: withArt ? ASPECTS[platformId] : null,
    });
  }
  const order = new Map(PLATFORMS.map(([id], i) => [id as string, i]));
  games.sort(
    (a, b) =>
      order.get(a.platformId)! - order.get(b.platformId)! ||
      a.title.replace(/^the /i, "").localeCompare(b.title.replace(/^the /i, "")),
  );
  return {
    platforms: PLATFORMS.map(([id, name, shortName]) => ({
      id,
      name,
      shortName,
      count: games.filter((g) => g.platformId === id).length,
      artAspect: null,
    })),
    games,
  };
}

function makeSettings(lib: Library): Settings {
  return {
    emulators: [
      { id: "retroarch", name: "RetroArch", exePath: "E:\\Emulators\\RetroArch\\retroarch.exe", exeExists: true, defaultTemplate: '"{emulator}" -L "{core}" -f "{rom}"', templateOverride: null, supportsM3u: true },
      { id: "dolphin", name: "Dolphin", exePath: null, exeExists: false, defaultTemplate: '"{emulator}" -b -e "{rom}"', templateOverride: null, supportsM3u: true },
    ],
    platforms: PLATFORMS.map(([id, name, shortName]) => ({
      id,
      name,
      shortName,
      emulatorId: "retroarch",
      defaultCore: "mgba",
      coreOverride: null,
      templateOverride: null,
      defaultExtensions: ["iso", "chd"],
      extensionsOverride: null,
      enabled: true,
      gameCount: lib.platforms.find((p) => p.id === id)?.count ?? 0,
    })),
    sources: [
      { id: 1, path: "E:\\Emulator Games\\PS2", platformId: "ps2", recursive: true, kind: "files", exists: true },
      { id: 2, path: "E:\\Old Drive\\SNES", platformId: "snes", recursive: true, kind: "files", exists: false },
    ],
    values: { retroarchCoresDir: "E:\\Emulators\\RetroArch\\cores" },
    scanning: false,
  };
}

export function installMockIpc() {
  const n = Number(new URLSearchParams(location.search).get("games") ?? 5000);
  const lib = makeLibrary(n);
  const settings = makeSettings(lib);
  mockWindows("main");
  mockIPC((cmd, args) => {
    switch (cmd) {
      case "get_library":
        return lib;
      case "get_settings":
        return settings;
      case "get_game": {
        const g = lib.games.find((x) => x.id === (args as { id: number }).id)!;
        return { ...g, serial: null, launchPath: `E:\\Games\\${g.title}.iso`, totalSize: 1.4e9, files: [] };
      }
      case "preview_launch":
        return ["E:\\Emulators\\RetroArch\\retroarch.exe", "-L", "cores\\mgba_libretro.dll", "-f", "E:\\Games\\x.iso"];
      case "list_dir": {
        const p = (args as { path: string | null }).path;
        if (!p) return { path: null, parent: null, truncated: false, entries: [{ name: "C:\\", path: "C:\\", isDir: true }, { name: "E:\\", path: "E:\\", isDir: true }] };
        const names = ["Emulator Games", "Emulators", "LaunchBox", "SteamLibrary"];
        return {
          path: p,
          parent: p.length > 3 ? p.replace(/\\[^\\]+\\?$/, "") || null : null,
          truncated: false,
          entries: names.map((n) => ({ name: n, path: `${p.replace(/\\$/, "")}\\${n}`, isDir: true })),
        };
      }
      default:
        return null;
    }
  });
  console.info(`[mock] Tauri IPC mocked with ${n} games`);
}
