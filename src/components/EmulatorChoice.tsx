import { Picker } from "./Picker";
import type { EmulatorOption, GameEmulators } from "../api";

/** "RetroArch · Achievements", "Azahar · No achievements · not set up". */
export function emulatorLabel(o: EmulatorOption): string {
  const bits = [o.name];
  if (o.achievements === true) bits.push("Achievements");
  if (o.achievements === false) bits.push("No achievements");
  if (!o.ready) bits.push("not set up");
  return bits.join(" · ");
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

/** "Play with…": starts the game with another emulator, this time only. */
export function PlayWith(props: { emus: GameEmulators; onPick: (emulator: string) => void; class?: string }) {
  return (
    <Picker
      title="Play with (this time only)"
      label="Play with…"
      class={props.class}
      value=""
      options={props.emus.options.map((o) => ({ value: o.id, label: emulatorLabel(o) }))}
      onChange={props.onPick}
    />
  );
}
