import type { JSX } from "solid-js";

// 3D models of the consoles the cartridge goes into, built from boxes and extruded outlines (like the cases):
// no pictures and no logos. 100 units = the width of the cartridge case's box (--w). The origin of each
// model is the middle of its top edge; `+x` right, `+y` down, `+z` towards the viewer.

const u = (n: number) => `calc(var(--k) * ${n})`;

/** A flat detail on a face; positions and sizes are % of that face. */
const P = (p: { l: number; t: number; w: number; h: number; bg: string; r?: string; ring?: string; shadow?: string }) => (
  <i
    class="pt"
    style={{
      left: `${p.l}%`,
      top: `${p.t}%`,
      width: `${p.w}%`,
      height: `${p.h}%`,
      background: p.bg,
      "border-radius": p.r,
      "box-shadow": p.ring ? `inset 0 0 0 ${u(0.7)} ${p.ring}` : p.shadow,
    }}
  />
);

type Face = { bg?: string; el?: JSX.Element };

/** A box with six faces: `x`/`z` are its centre, `y` its top edge. */
function Cuboid(p: { x: number; y: number; z: number; w: number; h: number; d: number; c: string; front?: Face; top?: Face }) {
  const z = (n: number) => `translateZ(${u(n)})`;
  return (
    <div class="cb" style={{ left: u(p.x - p.w / 2), top: u(p.y), width: u(p.w), height: u(p.h), transform: z(p.z) }}>
      <div class="cf ff" style={{ background: p.front?.bg ?? p.c, transform: z(p.d / 2) }}>{p.front?.el}</div>
      <div class="cf fb" style={{ background: p.c, transform: `rotateY(180deg) ${z(p.d / 2)}` }} />
      <div class="cf fl" style={{ background: p.c, width: u(p.d), left: u((p.w - p.d) / 2), transform: `rotateY(-90deg) ${z(p.w / 2)}` }} />
      <div class="cf fr" style={{ background: p.c, width: u(p.d), left: u((p.w - p.d) / 2), transform: `rotateY(90deg) ${z(p.w / 2)}` }} />
      <div class="cf ft" style={{ background: p.top?.bg ?? p.c, height: u(p.d), top: u((p.h - p.d) / 2), transform: `rotateX(90deg) ${z(p.h / 2)}` }}>{p.top?.el}</div>
      <div class="cf fu" style={{ background: p.c, height: u(p.d), top: u((p.h - p.d) / 2), transform: `rotateX(-90deg) ${z(p.h / 2)}` }} />
    </div>
  );
}

/** A rounded outline pushed back into a solid; `front` is drawn on its front face. `tilt` leans it back from its bottom edge. */
function Slab(p: { x: number; y: number; z: number; w: number; h: number; d: number; r: number; c: string; front?: JSX.Element; tilt?: number; layers?: number; edge?: string }) {
  const n = p.layers ?? 12;
  return (
    <div
      class="cb"
      style={{
        left: u(p.x - p.w / 2),
        top: u(p.y),
        width: u(p.w),
        height: u(p.h),
        "transform-origin": "50% 100%",
        transform: `translateZ(${u(p.z)}) rotateX(${-(p.tilt ?? 0)}deg)`,
      }}
    >
      {Array.from({ length: n }, (_, i) => (
        <i
          class="lay"
          style={{
            background: i === 0 ? p.c : (p.edge ?? p.c),
            "border-radius": u(p.r),
            filter: i === 0 ? undefined : `brightness(${0.74 + 0.1 * (1 - i / n)})`,
            transform: `translateZ(${u(p.d / 2 - (i * p.d) / (n - 1))})`,
          }}
        >
          {i === 0 && p.front}
        </i>
      ))}
    </div>
  );
}

// ------------------------------------------------------------------ the consoles

const SNES = () => (
  <>
    {/* the main body: two controller ports and the eject strip on the front, vents under it */}
    <Cuboid
      x={0} y={0} z={0} w={118} h={33} d={107} c="#d4d6db"
      front={{
        bg: "linear-gradient(#dcdde2, #cfd1d6)",
        el: (
          <>
            {P({ l: 43, t: 0, w: 14, h: 100, bg: "#b9bbc1" })}
            {P({ l: 12, t: 38, w: 26, h: 24, bg: "#2a2a2f", r: "12%" })}
            {P({ l: 62, t: 38, w: 26, h: 24, bg: "#2a2a2f", r: "12%" })}
            {P({ l: 0, t: 82, w: 100, h: 18, bg: "repeating-linear-gradient(90deg, #c2c4ca 0 3px, #d4d6db 3px 7px)" })}
          </>
        ),
      }}
    />
    {/* the raised back with the cartridge slot on top */}
    <Cuboid
      x={0} y={-7} z={-27.5} w={82} h={7} d={40} c="#cfd1d7"
      top={{
        bg: "linear-gradient(#cfd1d7, #cfd1d7)",
        el: (
          <>
            {P({ l: 7, t: 14, w: 86, h: 30, bg: "#131316", r: "6%" })}
            {P({ l: 7, t: 52, w: 86, h: 8, bg: "#b8bac0" })}
          </>
        ),
      }}
    />
    {/* Power and Reset */}
    <Cuboid x={-30} y={-3} z={27} w={22} h={3} d={9} c="#8d82b8" />
    <Cuboid x={30} y={-3} z={27} w={22} h={3} d={9} c="#8d82b8" />
  </>
);

const N64 = () => (
  <>
    <Cuboid
      x={0} y={0} z={0} w={136} h={30} d={96} c="#3a3b40"
      front={{
        bg: "linear-gradient(#45464b, #333438)",
        el: <>{P({ l: 39, t: 18, w: 22, h: 56, bg: "#101012", r: "4%", ring: "#5b5c62" })}</>,
      }}
    />
    {/* the two front corners stand taller, with a pair of controller ports each */}
    {[-48, 48].map((x) => (
      <Cuboid
        x={x} y={0} z={38} w={40} h={39} d={38} c="#36373c"
        front={{
          bg: "linear-gradient(#424348, #303135)",
          el: (
            <>
              {P({ l: 8, t: 28, w: 38, h: 40, bg: "#18181b", r: "30%", ring: "#8d8e94" })}
              {P({ l: 54, t: 28, w: 38, h: 40, bg: "#18181b", r: "30%", ring: "#8d8e94" })}
            </>
          ),
        }}
      />
    ))}
    {/* the cartridge bay at the back: the slot, then the vents */}
    <Cuboid
      x={0} y={-9} z={-40} w={76} h={9} d={30} c="#2d2e32"
      top={{
        bg: "#2d2e32",
        el: (
          <>
            {P({ l: 6, t: 8, w: 88, h: 34, bg: "#070708", r: "5%" })}
            {P({ l: 6, t: 50, w: 88, h: 42, bg: "repeating-linear-gradient(0deg, #1b1c1f 0 3px, #38393e 3px 7px)" })}
          </>
        ),
      }}
    />
  </>
);

const GBA = () => (
  <Slab
    x={0} y={0} z={0} w={154} h={88} d={27} r={14} c="#5648a6" edge="#4a3e93"
    front={
      <>
        {P({ l: 25, t: 8, w: 50, h: 68, bg: "#24212d", r: "3%" })}
        {P({ l: 29, t: 14, w: 42, h: 56, bg: "#5b685c", r: "1%" })}
        {P({ l: 7, t: 43, w: 14, h: 8, bg: "#2c2840", r: "30%" })}
        {P({ l: 10, t: 30, w: 8, h: 33, bg: "#2c2840", r: "30%" })}
        {P({ l: 83, t: 33, w: 8, h: 14, bg: "#6a5cc0", r: "50%", ring: "#2c2840" })}
        {P({ l: 74, t: 50, w: 8, h: 14, bg: "#6a5cc0", r: "50%", ring: "#2c2840" })}
        {P({ l: 33, t: 84, w: 12, h: 4, bg: "#2c2840", r: "50%" })}
        {P({ l: 55, t: 84, w: 12, h: 4, bg: "#2c2840", r: "50%" })}
      </>
    }
  />
);

/** A DS or 3DS: two halves on a hinge, the upper one leaning back. */
const Clamshell = (p: { shell: string; edge: string; inner: string; bezel: string; screen: string }) => (
  <>
    <Slab
      x={0} y={0} z={-2} w={169} h={88} d={26} r={9} c={p.shell} edge={p.edge} tilt={9}
      front={
        <>
          {P({ l: 3, t: 5, w: 94, h: 90, bg: p.inner, r: "4%" })}
          {P({ l: 24, t: 11, w: 52, h: 78, bg: p.bezel, r: "2%" })}
          {P({ l: 26.5, t: 14, w: 47, h: 72, bg: p.screen })}
        </>
      }
    />
    <Slab
      x={0} y={92} z={0} w={169} h={88} d={26} r={9} c={p.shell} edge={p.edge}
      front={
        <>
          {P({ l: 22, t: 8, w: 52, h: 84, bg: p.bezel, r: "2%" })}
          {P({ l: 24.5, t: 12, w: 47, h: 76, bg: p.screen })}
          {P({ l: 6, t: 40, w: 11, h: 7, bg: "#1e1f24", r: "30%" })}
          {P({ l: 9, t: 30, w: 5, h: 27, bg: "#1e1f24", r: "30%" })}
          {P({ l: 85, t: 36, w: 5, h: 10, bg: "#1e1f24", r: "50%" })}
          {P({ l: 79, t: 46, w: 5, h: 10, bg: "#1e1f24", r: "50%" })}
          {P({ l: 91, t: 46, w: 5, h: 10, bg: "#1e1f24", r: "50%" })}
          {P({ l: 85, t: 56, w: 5, h: 10, bg: "#1e1f24", r: "50%" })}
        </>
      }
    />
  </>
);

const SWITCH = () => (
  <>
    <Slab
      x={0} y={0} z={0} w={190} h={110} d={13} r={4} c="#16171a" layers={8}
      front={<>{P({ l: 3.5, t: 5, w: 93, h: 90, bg: "#07080a", r: "1.5%" })}</>}
    />
    <Slab
      x={-113} y={-1} z={0} w={37} h={112} d={16} r={13} c="#12b8e4" edge="#0d9dc4" layers={8}
      front={
        <>
          {P({ l: 25, t: 14, w: 50, h: 15, bg: "#2a2b30", r: "50%" })}
          {P({ l: 20, t: 52, w: 12, h: 8, bg: "#2a2b30", r: "50%" })}
          {P({ l: 44, t: 52, w: 12, h: 8, bg: "#2a2b30", r: "50%" })}
          {P({ l: 32, t: 62, w: 12, h: 8, bg: "#2a2b30", r: "50%" })}
          {P({ l: 32, t: 44, w: 12, h: 8, bg: "#2a2b30", r: "50%" })}
        </>
      }
    />
    <Slab
      x={113} y={-1} z={0} w={37} h={112} d={16} r={13} c="#ff4757" edge="#d93546" layers={8}
      front={
        <>
          {P({ l: 25, t: 56, w: 50, h: 15, bg: "#2a2b30", r: "50%" })}
          {P({ l: 32, t: 16, w: 12, h: 8, bg: "#2a2b30", r: "50%" })}
          {P({ l: 56, t: 24, w: 12, h: 8, bg: "#2a2b30", r: "50%" })}
          {P({ l: 32, t: 32, w: 12, h: 8, bg: "#2a2b30", r: "50%" })}
          {P({ l: 8, t: 24, w: 12, h: 8, bg: "#2a2b30", r: "50%" })}
        </>
      }
    />
  </>
);

const MODELS: Record<string, () => JSX.Element> = {
  snes: SNES,
  n64: N64,
  gba: GBA,
  nds: () => Clamshell({ shell: "#2a2b30", edge: "#222328", inner: "#1a1b1f", bezel: "#121316", screen: "#6e7379" }),
  n3ds: () => Clamshell({ shell: "#1fa9cc", edge: "#188bab", inner: "#17181c", bezel: "#101114", screen: "#707378" }),
  switch: SWITCH,
};

export const hasConsole = (k: string) => k in MODELS;

/** The console a cartridge goes into; the cartridge's flat plane (z = 0 of the case) lines up with its slot (`--cz`). */
export function ConsoleModel(props: { k: string }) {
  return <div class="cm">{MODELS[props.k]?.()}</div>;
}
