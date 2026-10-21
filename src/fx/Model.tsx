import { createEffect, createSignal, Show } from "solid-js";
import { artUrl, type GameRow } from "../api";
import { hue } from "../components/GameCard";
import { CART_SVG, SLOT_SVG } from "./hardware";

type Media = "cd" | "dvd" | "bd" | "cart";
/** Each platform's case: media, model key (fx.css), size in stage px [w, h, depth], and disc variant. */
const CASES: Record<string, { m: Media; k: string; size: [number, number, number]; disc?: "mini" | "umd" }> = {
  ps1: { m: "cd", k: "ps1", size: [150, 132, 12] },
  saturn: { m: "cd", k: "saturn", size: [150, 132, 12] },
  segacd: { m: "cd", k: "segacd", size: [150, 132, 12] },
  ps2: { m: "dvd", k: "ps2", size: [134, 190, 14] },
  wii: { m: "dvd", k: "wii", size: [134, 190, 14] },
  xbox: { m: "dvd", k: "xbox", size: [134, 190, 14] },
  xbox360: { m: "dvd", k: "x360", size: [134, 190, 14] },
  gc: { m: "dvd", k: "gc", size: [128, 180, 14], disc: "mini" },
  psp: { m: "dvd", k: "psp", size: [108, 175, 13], disc: "umd" },
  ps3: { m: "bd", k: "ps3", size: [134, 168, 11] },
  snes: { m: "cart", k: "snes", size: [186, 132, 34] },
  n64: { m: "cart", k: "n64", size: [186, 132, 34] },
  gba: { m: "cart", k: "gba", size: [112, 128, 24] },
  nds: { m: "cart", k: "nds", size: [124, 138, 14] },
  "3ds": { m: "cart", k: "n3ds", size: [124, 138, 14] },
  switch: { m: "cart", k: "switch", size: [104, 170, 10] },
};
/** Spine colours [spine, band] per platform (the band carries the platform's name; no logos). */
const SPINES: Record<string, [string, string]> = {
  ps1: ["#1b1c20", "#4a4f66"], saturn: ["#1b1c20", "#3a3d4a"], segacd: ["#1b1c20", "#1a56a8"],
  ps2: ["#0c0d0f", "#1f4fb4"], wii: ["#eef0f2", "#27a7d8"], xbox: ["#226026", "#6fd12a"], x360: ["#409e38", "#5db32f"],
  gc: ["#1b1b21", "#5547a6"], psp: ["#1b1d22", "#8d96a3"], ps3: ["#183f8f", "#0b1b3f"], snes: ["#8a8d99", "#5b5f78"],
  n64: ["#2c2f36", "#c23b34"], gba: ["#3b2f26", "#4d3fa0"], nds: ["#cfd6dc", "#2f3b6e"], n3ds: ["#eceef1", "#2aa0d8"],
  switch: ["#d6102a", "#a50c20"],
};
const OTHER = { m: "dvd" as Media, k: "other", size: [134, 190, 14] as [number, number, number] };

export const caseOf = (platformId: string) => CASES[platformId] ?? OTHER;

/**
 * A game's case in 3D (six faces at real proportions): a disc case whose lid
 * opens on the tray, or a cartridge box with the cartridge and the console
 * slot it goes into (`launch`). The cover is the front; LaunchBox's disc and
 * back pictures are used when KB has them (`detail`), else drawn stand-ins.
 */
export function Model(props: {
  game: GameRow;
  platformName: string;
  /** Stage px per model px. */
  scale?: number;
  /** Load the back cover and disc pictures (the focused or launching case). */
  detail?: boolean;
  /** Include the cartridge and the console slot (the launch animation). */
  launch?: boolean;
  class?: string;
  ref?: (el: HTMLDivElement) => void;
}) {
  const c = () => caseOf(props.game.platformId);
  const k = () => props.scale ?? 1;
  /**
   * The case takes the cover's own shape (same area as the platform's usual
   * case), so nothing is cropped. A cartridge being launched keeps the usual
   * shape: the cartridge and console are drawn to fit it.
   */
  const dims = (): [number, number, number] => {
    const [w, h, d] = c().size;
    const a = props.game.art > 0 ? props.game.artAspect : null;
    if (!a || (c().m === "cart" && props.launch)) return [w, h, d];
    const asp = Math.min(1.8, Math.max(0.55, a));
    const hh = Math.sqrt(w * h * asp);
    return [hh / asp, hh, d];
  };
  const spine = () => SPINES[c().k] ?? ["", ""];
  const h = () => hue(props.game.title);
  const fallback = () => `linear-gradient(160deg, hsl(${h()} 55% 44%), hsl(${(h() + 40) % 360} 50% 14%))`;
  const year = () => (props.game.year ? ` · ${props.game.year}` : "");
  // The back and disc pictures can still be downloading when a case first asks
  // for them (LaunchBox's pass runs in the background), so a miss is retried
  // the next time the case is looked at.
  const [backOk, setBackOk] = createSignal(true);
  const [discOk, setDiscOk] = createSignal(true);
  createEffect(() => {
    if (!props.detail) {
      setBackOk(true);
      setDiscOk(true);
    }
  });
  // Spine title size: as large as the spine's width allows, smaller when the title is long.
  const spineFont = () => {
    const [, h, d] = dims();
    return Math.min(d * 0.5, (h * 0.7) / (Math.max(props.game.title.length, 6) * 0.55)) * k();
  };
  const Spine = () => (
    <div class="spine">
      <i class="sband">{props.platformName}</i>
      <b class="stitle" style={{ "font-size": `${spineFont()}px` }}>{props.game.title}</b>
    </div>
  );
  const Art = (p: { big?: boolean }) => (
    <Show
      when={props.game.art > 0}
      fallback={
        <div class="art" style={{ background: fallback() }}>
          <i>{props.platformName}</i>
          <b>{props.game.title}</b>
        </div>
      }
    >
      <img class="art-img" src={artUrl(props.game.id, props.game.art, p.big ? "f" : "t")} alt="" draggable={false} decoding="async" />
    </Show>
  );
  const box = (front: boolean) => (
    <div class="box">
      <div class="f back">
        <Show
          when={props.detail && backOk()}
          fallback={
            <div class="bart">
              <b>{props.game.title}</b>
              <span>
                {props.platformName}
                {year()}
                {props.game.developer ? ` · ${props.game.developer}` : ""}
              </span>
              <div class="shots"><i /><i /><i /></div>
              <div class="lines"><i /><i /><i style={{ width: "70%" }} /></div>
            </div>
          }
        >
          <img class="art-img" src={artUrl(props.game.id, 0, "b")} alt="" draggable={false} onError={() => setBackOk(false)} />
        </Show>
      </div>
      <div class="f sl"><Spine /></div>
      <div class="f sr"><Spine /></div>
      <div class="f st" />
      <div class="f sb" />
      <Show when={front}>
        <div class="f bf">
          <Art big={props.launch} />
          <div class="sheen" />
        </div>
      </Show>
    </div>
  );
  return (
    <div
      ref={props.ref}
      class={`m3 ${c().m} pf-${c().k} ${props.class ?? ""}`}
      style={{
        "--w": `${dims()[0] * k()}px`,
        "--h": `${dims()[1] * k()}px`,
        "--d": `${dims()[2] * k()}px`,
        "--hue": h(),
        "--spine": spine()[0] || undefined,
        "--band": spine()[1] || undefined,
      }}
    >
      <Show
        when={c().m === "cart"}
        fallback={
          <>
            {box(false)}
            <div class="tray">
              <div class={`dwrap ${c().disc ?? ""}`}>
                <div class="disc">
                  <Show
                    when={props.detail && discOk()}
                    fallback={
                      <div class="print" style={{ background: props.game.art > 0 ? undefined : fallback() }}>
                        <Show when={props.game.art > 0} fallback={<b>{props.game.title}</b>}>
                          <img class="art-img" src={artUrl(props.game.id, props.game.art, "t")} alt="" draggable={false} />
                        </Show>
                      </div>
                    }
                  >
                    <img class="disc-img" src={artUrl(props.game.id, 0, "d")} alt="" draggable={false} onError={() => setDiscOk(false)} />
                  </Show>
                </div>
              </div>
            </div>
            <div class="lid">
              <div class="f lf">
                <Art big={props.launch} />
                <div class="sheen" />
              </div>
              <div class="f lb" />
            </div>
          </>
        }
      >
        {box(true)}
        <Show when={props.launch}>
          <div class={`cartridge ${CART_SVG[c().k] ? "drawn" : ""}`}>
            <Show when={CART_SVG[c().k]}>{(svg) => <div class="hw" innerHTML={svg()} />}</Show>
            <div class="clabel" style={{ background: props.game.art > 0 ? undefined : fallback() }}>
              <Show when={props.game.art > 0} fallback={<b>{props.game.title}</b>}>
                <img class="art-img" src={artUrl(props.game.id, props.game.art, "t")} alt="" draggable={false} />
              </Show>
            </div>
          </div>
          <div class={`slot ${SLOT_SVG[c().k] ? "drawn" : ""}`}>
            <Show when={SLOT_SVG[c().k]}>{(svg) => <div class="hw" innerHTML={svg()} />}</Show>
            <span class="led" />
          </div>
        </Show>
      </Show>
    </div>
  );
}
