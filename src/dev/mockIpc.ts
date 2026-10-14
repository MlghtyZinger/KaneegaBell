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

const GENRES = ["Adventure", "Platform", "Role-playing (RPG)", "Shooter", "Strategy", "Racing", "Fighting"];

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
      genres: r() < 0.7 ? [1 + Math.floor(r() * GENRES.length)] : undefined,
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
    genres: GENRES.map((name, i) => ({ id: i + 1, name })),
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
        const genre = lib.genres.find((x) => g.genres?.includes(x.id));
        return {
          ...g,
          serial: null,
          launchPath: `E:\\Games\\${g.title}.iso`,
          totalSize: 1.4e9,
          files: [],
          artSource: null,
          meta: genre
            ? {
                title: g.title,
                description: `${g.title} is a made-up game for layout work.\nIt has a second paragraph, too.`,
                releaseDate: 1_158_019_200,
                developer: "Mock Studio",
                publisher: "Mock Publishing",
                genres: [genre.name],
                igdbId: 1000 + g.id,
                matchStatus: "ok",
                slug: g.title.toLowerCase().replace(/[^a-z0-9]+/g, "-"),
              }
            : { title: null, description: null, releaseDate: null, developer: null, publisher: null, genres: [], igdbId: null, matchStatus: "notfound", slug: null },
        };
      }
      case "get_services": {
        const q = new URLSearchParams(location.search);
        return { igdb: q.get("igdb") === "1", igdbClientId: "abc123…", raUser: q.get("ra") === "1" ? "MockUser" : null };
      }
      case "get_achievements": {
        // ?ra=1: odd ids are recognized with progress, even ids aren't; 3DS is unsupported.
        if (new URLSearchParams(location.search).get("ra") !== "1") return { status: "unsupported", hash: null, progress: null, fetchedAt: null, stale: false, raTitle: null, raGameId: null, accepted: [] };
        const id = (args as { id: number }).id;
        const g = lib.games.find((x) => x.id === id)!;
        if (g.platformId === "3ds") return { status: "unsupported", hash: null, progress: null, fetchedAt: null, stale: false, raTitle: null, raGameId: null, accepted: [] };
        if (id % 4 === 0) return { status: "noset", hash: "0123456789abcdef0123456789abcdef", progress: null, fetchedAt: null, stale: false, raTitle: null, raGameId: null, accepted: [] };
        if (id % 2 === 0)
          return {
            status: "unknown", hash: "0123456789abcdef0123456789abcdef", progress: null, fetchedAt: null, stale: false,
            raTitle: g.title, raGameId: 2774,
            accepted: [
              { name: `${g.title} (USA) (Debug Patch) (Bartis1989)`, patched: true },
              { name: `${g.title} (Europe) (En,Fr,De,Es,It)`, patched: false },
            ],
          };
        const r = rng(id);
        const achievements = Array.from({ length: 24 }, (_, i) => {
          const earned = r() < 0.4;
          return {
            id: i + 1,
            title: `${WORDS[Math.floor(r() * WORDS.length)]} ${WORDS[Math.floor(r() * WORDS.length)]}`,
            description: "Do something notable in the game, perhaps twice, without taking damage.",
            points: [1, 2, 3, 5, 10, 25, 50][Math.floor(r() * 7)],
            badge: String(10000 + i),
            earned: earned ? "2025-03-14 18:22:05" : null,
            hardcore: earned && r() < 0.5,
            kind: r() < 0.1 ? "missable" : null,
            rarity: r(),
          };
        });
        const got = achievements.filter((a) => a.earned);
        return {
          status: "ok",
          hash: "fedcba9876543210fedcba9876543210",
          fetchedAt: Math.floor(Date.now() / 1000),
          stale: false,
          raTitle: g.title,
          raGameId: 1000 + id,
          accepted: [],
          progress: {
            raGameId: 1000 + id,
            title: g.title,
            icon: null,
            total: achievements.length,
            earned: got.length,
            earnedHardcore: got.filter((a) => a.hardcore).length,
            pointsTotal: achievements.reduce((s, a) => s + a.points, 0),
            pointsEarned: got.reduce((s, a) => s + a.points, 0),
            achievements,
          },
        };
      }
      case "igdb_search":
        return [
          { igdbId: 1, name: "Okami", year: 2006, cover: null, onPlatform: true, summary: "Play as Amaterasu, the sun goddess, in wolf form." },
          { igdbId: 2, name: "Okami HD", year: 2012, cover: null, onPlatform: false, summary: "A remaster." },
        ];
      case "get_storage":
        return { downloadedBytes: 182_452_224, dataDir: "C:\\Users\\you\\AppData\\Roaming\\com.kaneegabell.kb" };
      case "clear_downloaded":
        return 182_452_224;
      case "list_displays":
        return [
          { id: "\\\\?\\DISPLAY#GSM5C7C", name: "LG ULTRAGEAR+", width: 2560, height: 1440, scale: 1.5, primary: true, current: true },
          { id: "\\\\?\\DISPLAY#SNYE405", name: "SONY TV", width: 3840, height: 2160, scale: 1, primary: false, current: false },
        ];
      case "tv_enter":
        return { displayMissing: false };
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
