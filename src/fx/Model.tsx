import { createEffect, createResource, createSignal, Show } from "solid-js";
import { api, artUrl, type GameRow } from "../api";
import { caseRev } from "../state/fx";
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

/** Edge colours per cover version, asked for once (the shelf shows the same cases again and again). */
const edgeCache = new Map<string, Promise<[string, string] | null>>();
function edgeColours(key: string, id: number) {
  let p = edgeCache.get(key);
  if (!p) {
    p = api.coverEdges(id).catch(() => null);
    edgeCache.set(key, p);
  }
  return p;
}

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
    const asp = Math.min(2.5, Math.max(0.4, a));
    const hh = Math.sqrt(w * h * asp);
    // A tall spine picture sets how thick the case is (within reason), so it isn't squashed to fit.
    const dd = spineOk() && spineRatio() > 0 && !spineWide() ? Math.min(d * 1.6, Math.max(d * 0.75, hh * spineRatio())) : d;
    return [hh / asp, hh, dd];
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
  const [spineOk, setSpineOk] = createSignal(true);
  /** The game's own cartridge picture (LaunchBox) for the launch animation; else the drawn cartridge. */
  const [cartOk, setCartOk] = createSignal(true);
  /** The spine picture's shape: LaunchBox's wide ones are for a horizontal box's top edge. */
  const [spineWide, setSpineWide] = createSignal(false);
  /** Width / height of a tall spine picture. */
  const [spineRatio, setSpineRatio] = createSignal(0);
  createEffect(() => {
    caseRev();
    if (!props.detail || caseRev() > 0) {
      setBackOk(true);
      setDiscOk(true);
      setSpineOk(true);
      setCartOk(true);
    }
  });
  // The top and bottom faces: the cover's own edge colour (else the platform's), like the printed wrap of a real box;
  // a horizontal box's top spine picture goes on them, and a tall spine picture wraps round onto them turned on its side.
  const [edges] = createResource(
    () => (props.game.art > 0 ? `${props.game.id}:${props.game.art}` : null),
    (key) => edgeColours(key, props.game.id),
  );
  const Edge = (p: { at: 0 | 1 }) => (
    <div class="edge" style={{ "background-color": edges()?.[p.at] }}>
      <Show when={spineOk() && spineWide()}>
        <img class="spine-img" src={artUrl(props.game.id, caseRev(), "s")} alt="" draggable={false} />
      </Show>
      <Show when={spineOk() && !spineWide() && spineRatio() > 0}>
        <img class="spine-img tall" classList={{ flip: p.at === 1 }} src={artUrl(props.game.id, caseRev(), "s")} alt="" draggable={false} />
      </Show>
    </div>
  );
  // Spine title size: as large as the spine's width allows, smaller when the title is long.
  const spineFont = () => {
    const [, h, d] = dims();
    return Math.min(d * 0.5, (h * 0.7) / (Math.max(props.game.title.length, 6) * 0.55)) * k();
  };
  // LaunchBox's own spine picture when it has one; else the platform's colour with its name and the title.
  const Spine = () => (
    <Show
      when={spineOk()}
      fallback={
        <div class="spine drawn">
          <i class="sband">{props.platformName}</i>
          <b class="stitle" style={{ "font-size": `${spineFont()}px` }}>{props.game.title}</b>
        </div>
      }
    >
      <div class="spine">
        <img
          class="spine-img"
          classList={{ wide: spineWide() }}
          src={artUrl(props.game.id, caseRev(), "s")}
          alt=""
          draggable={false}
          onLoad={(e) => {
            const { naturalWidth: w, naturalHeight: h } = e.currentTarget;
            setSpineWide(w > h);
            setSpineRatio(w > h ? 0 : w / h);
          }}
          onError={() => setSpineOk(false)}
        />
      </div>
    </Show>
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
  // A cartridge's launch keeps the platform's shape for the cartridge and the console, but its box takes the cover's own
  // shape (same area), so the cover fills it with no bars and no stretching.
  const launchBox = () => {
    if (!(c().m === "cart" && props.launch) || !(props.game.art > 0)) return undefined;
    const [w, h] = c().size;
    const asp = Math.min(2.5, Math.max(0.4, props.game.artAspect ?? 1));
    const hh = Math.sqrt(w * h * asp);
    return { "--w": `${(hh / asp) * k()}px`, "--h": `${hh * k()}px` };
  };
  const box = (front: boolean) => (
    <div class="box" classList={{ own: !!launchBox() }} style={launchBox()}>
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
          <img class="art-img" src={artUrl(props.game.id, caseRev(), "b")} alt="" draggable={false} onError={() => setBackOk(false)} />
        </Show>
      </div>
      <div class="f sl"><Spine /></div>
      <div class="f sr"><Spine /></div>
      <div class="f st"><Edge at={0} /></div>
      <div class="f sb"><Edge at={1} /></div>
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
                    <img class="disc-img" src={artUrl(props.game.id, caseRev(), "d")} alt="" draggable={false} onError={() => setDiscOk(false)} />
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
          <Show
            when={props.detail && cartOk()}
            fallback={
              <div class={`cartridge ${CART_SVG[c().k] ? "drawn" : ""}`}>
                <Show when={CART_SVG[c().k]}>{(svg) => <div class="hw" innerHTML={svg()} />}</Show>
                <div class="clabel" style={{ background: props.game.art > 0 ? undefined : fallback() }}>
                  <Show when={props.game.art > 0} fallback={<b>{props.game.title}</b>}>
                    <img class="art-img" src={artUrl(props.game.id, props.game.art, "t")} alt="" draggable={false} />
                  </Show>
                </div>
              </div>
            }
          >
            <div class={`cartridge real ${CART_SVG[c().k] ? "drawn" : ""}`}>
              <img class="cart-img" src={artUrl(props.game.id, caseRev(), "d")} alt="" draggable={false} onError={() => setCartOk(false)} />
            </div>
          </Show>
          <div class={`slot ${SLOT_SVG[c().k] ? "drawn" : ""}`}>
            <Show when={SLOT_SVG[c().k]}>{(svg) => <div class="hw" innerHTML={svg()} />}</Show>
            <span class="led" />
          </div>
        </Show>
      </Show>
    </div>
  );
}
