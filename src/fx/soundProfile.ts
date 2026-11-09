export type SoundRole = "move" | "select" | "back" | "error" | "disc" | "cart";
export const MENU_SOUNDS = [
  { value: "kb", label: "KB Console" },
];

export function soundProfile(tv: boolean, layout: string, theme: string, _choice: string): string {
  const style = tv && layout !== "kb" ? layout : theme;
  return ["ps2", "ps3", "wii"].includes(style) ? style : "kb";
}

export function soundName(profile: string, role: SoundRole): string | null {
  if (["ps2", "ps3", "wii"].includes(profile) && ["error", "cart", "disc"].includes(role)) return null;
  return `menu_${profile}_${role}`;
}
