export const PC_LAUNCHERS = [
  { id: "steam", name: "Steam" }, { id: "epic", name: "Epic Games" },
  { id: "gog", name: "GOG Galaxy" }, { id: "battlenet", name: "Battle.net" },
  { id: "xboxpc", name: "Xbox PC" }, { id: "amazon", name: "Amazon Games" },
  { id: "ubisoft", name: "Ubisoft Connect" }, { id: "ea", name: "EA" },
] as const;
export const isLauncher = (id: string) => PC_LAUNCHERS.some((p) => p.id === id);
export const launcherName = (id: string) => PC_LAUNCHERS.find((p) => p.id === id)?.name ?? "Game client";
