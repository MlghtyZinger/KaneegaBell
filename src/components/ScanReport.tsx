import { createMemo, For, Show } from "solid-js";
import type { GameChange } from "../api";

/** Lines shown per platform before "…and N more" (a first scan can add thousands). */
const MAX_LINES = 200;

type Kind = GameChange["kind"];

const HEADINGS: Record<Kind, string> = {
  added: "Added",
  replaced: "New file (e.g. compressed); play time, favorites and picks kept",
  moved: "Moved to another platform",
  removed: "Removed (no longer on disk)",
  hidden: "Hidden (no longer on disk; kept because you played or favorited them)",
};
const ORDER: Kind[] = ["added", "replaced", "moved", "removed", "hidden"];

function label(c: GameChange): string {
  return c.variant ? `${c.title} (${c.variant})` : c.title;
}

/** One-line summary: "3 added to PS2, 1 to GBA · 2 removed". */
export function summarizeChanges(changes: GameChange[], name: (platformId: string) => string): string {
  const parts: string[] = [];
  for (const kind of ORDER) {
    const of = changes.filter((c) => c.kind === kind);
    if (of.length === 0) continue;
    if (kind === "added") {
      const per = new Map<string, number>();
      for (const c of of) per.set(c.platformId, (per.get(c.platformId) ?? 0) + 1);
      const list = [...per].map(([p, n], i) => (i === 0 ? `${n} added to ${name(p)}` : `${n} to ${name(p)}`));
      parts.push(list.join(", "));
    } else {
      parts.push(`${of.length} ${kind}`);
    }
  }
  return parts.join(" · ");
}

/**
 * Which games a scan (or removing a folder) added, removed or moved,
 * grouped by platform. Each group is a <details> so long lists stay folded
 * and a controller can open them one at a time.
 */
export function ScanReport(props: { changes: GameChange[]; name: (platformId: string) => string }) {
  const groups = createMemo(() =>
    ORDER.map((kind) => {
      const per = new Map<string, GameChange[]>();
      for (const c of props.changes) {
        if (c.kind !== kind) continue;
        const list = per.get(c.platformId) ?? [];
        list.push(c);
        per.set(c.platformId, list);
      }
      return { kind, platforms: [...per] };
    }).filter((g) => g.platforms.length > 0),
  );

  return (
    <div class="scan-report">
      <For each={groups()}>
        {(g) => (
          <div class={`scan-report-group ${g.kind}`}>
            <h3>{HEADINGS[g.kind]}</h3>
            <For each={g.platforms}>
              {([platform, games]) => (
                <details open={props.changes.length <= 12}>
                  <summary>
                    {props.name(platform)} <span class="dim">· {games.length}</span>
                  </summary>
                  <ul>
                    <For each={games.slice(0, MAX_LINES)}>
                      {(c) => (
                        <li>
                          {label(c)}
                          <Show when={c.kind === "moved" && c.fromPlatform}>
                            <span class="dim"> (was {props.name(c.fromPlatform!)})</span>
                          </Show>
                        </li>
                      )}
                    </For>
                    <Show when={games.length > MAX_LINES}>
                      <li class="dim">…and {games.length - MAX_LINES} more</li>
                    </Show>
                  </ul>
                </details>
              )}
            </For>
          </div>
        )}
      </For>
    </div>
  );
}
