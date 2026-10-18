import { Picker } from "./Picker";
import type { EmulatorOption, GameEmulators } from "../api";

/** "RetroArch · Achievements", "Azahar · No achievements · not set up". */
export function emulatorLabel(o: EmulatorOption): string {
  const bits = [o.name];
  if (o.achievements === true) bits.push("Achievements");
  if (o.achievements === false) bits.push("No achievements");
  if (!o.ready) bits.push("not set up");
  else if (o.warning) bits.push("BIOS missing?");
  return bits.join(" · ");
}

/** The warning of the emulator a plain Play would use (the game's pick, else the default). */
export function playWarning(e: GameEmulators | null | undefined): string | null {
  if (!e) return null;
  const id = e.chosen ?? e.defaultId;
  return e.options.find((o) => o.id === id)?.warning ?? null;
}

/**
 * "Launch with": the emulator this game always uses (saved), or the
 * platform's default.
 */
export function LaunchWith(props: { emus: GameEmulators; onChange: (emulator: string | null) => void }) {
  const defaultName = () => props.emus.options.find((o) => o.id === props.emus.defaultId)?.name ?? props.emus.defaultId;
  return (
    <Picker
      title="Always launch this game with"
      value={props.emus.chosen ?? ""}
      options={[
        { value: "", label: `Default (${defaultName()})` },
        ...props.emus.options.map((o) => ({ value: o.id, label: emulatorLabel(o) })),
      ]}
      onChange={(v) => props.onChange(v || null)}
    />
  );
}
