import { createEffect, createMemo, createSignal, For, Index, on, onCleanup, onMount, Show } from "solid-js";
import { artUrl, type GameRow } from "../api";
import { caseRev, fx } from "../state/fx";
import { formatLastPlayed, formatPlayTime, session } from "../state/session";
import { pushHandler } from "../input/actions";
import { sfx } from "../fx/audio";
import { introPlaying } from "../fx/Intro";
import { preparePs2Audio, syncPs2Audio } from "../fx/ps2Audio";
import { caseOf } from "../fx/Model";
import type { ConsoleEntry } from "./ConsoleMenus";

interface Props {
  count: number; focused: number; entry: (index: number) => ConsoleEntry | undefined;
  game: (index: number) => GameRow | undefined;
  select: (index: number) => void; play: () => void; more: () => void; cover: () => void;
  settings: () => void;
  launching: boolean; error: string | null;
  /** The library's tabs (platforms etc.), the current one, their names and how to switch; L1/R1 still step through them. */
  tabs: string[]; tab: string; tabLabel: (id: string) => string; chooseTab: (id: string) => void;
  /** The selected game's details (LaunchBox / IGDB), once loaded. */
  detail?: { description: string | null; releaseDate: number | null; developer: string | null; genres: string[] } | null;
}

/** Games shown either side of the selected one; the shelf slides as the selection moves. */
const REACH = 3;
/** Extrusion layers between the front and back faces (gives the object its thickness). */
const LAYERS = { disc: 5, cart: 9, cover: 4 };

/** The game's disc or cartridge (else its cover) as a 3D object: the picture extruded to a thickness, spinning when selected. */
function Object3d(props: { game?: GameRow; entry: ConsoleEntry }) {
  const [failed, setFailed] = createSignal(false);
  const kind = () => failed() || !props.game ? "cover" : caseOf(props.game.platformId).m === "cart" ? "cart" : "disc";
  const src = () => kind() === "cover" ? props.entry.picture : artUrl(props.game!.id, caseRev(), "d");
  return <span class={`p2o p2-${kind()}`} style={{ "--n": LAYERS[kind()] }}>
    <Show when={src()} fallback={<span class="p2-title">{props.entry.title}</span>}>{(url) =>
      <span class="p2spin">
        <Index each={Array.from({ length: LAYERS[kind()] })}>{(_, i) => <img class="p2l" style={{ "--i": i }} src={url()} alt="" decoding="async" />}</Index>
        <Show when={kind() === "disc"} fallback={<img class="p2b" src={url()} alt="" decoding="async" />}><span class="p2b" /></Show>
        <img class="p2f" src={url()} alt="" decoding="async" onError={() => kind() === "cover" ? undefined : setFailed(true)} />
      </span>
    }</Show>
  </span>;
}

const smooth = (x: number) => { x = Math.max(0, Math.min(1, x)); return x * x * (3 - 2 * x); };
/** A slow drift that never settles into a loop: two sines at unrelated rates, about -1..1. */
const drift = (t: number, a: number, b: number, p: number, q: number) => Math.sin(a * t + p) * .6 + Math.sin(b * t + q) * .4;
/**
 * Where orb `i` is at time `t`, in the intro's 1920x1080 space, relative to the ring's centre. The menu opens exactly on the
 * intro's ring, then the lights drift: the gap between them, how flat the path is, its size and its tilt each wander on their
 * own unrelated rhythms, so the lights sometimes circle, trail as an arc, line up and spin as a row, or bunch into one comet,
 * and everything in between, never in the same order. `sp` (the gap between neighbours) tells the comet from the rest.
 */
export const orbAt = (i: number, t: number) => {
  const ft = t - 8.8, u = t - 5.15, w = smooth(ft / 3);
  const ring = { sp: Math.PI / 4, flat: .72 + .08 * Math.sin(u * .6), R: 135, rot: 0, ph: u * 1.3 };
  const free = { sp: .04 + .76 * (.5 + .5 * drift(ft, .29, .171, 1.3, .4)), flat: .06 + .8 * (.5 + .5 * drift(ft, .23, .137, 2.1, 0)),
    R: 125 + 40 * drift(ft, .19, .097, .7, 2.2), rot: .21 * ft + .8 * Math.sin(.13 * ft), ph: u * 1.3 + .9 * Math.sin(.17 * u + .3) };
  const m = (k: keyof typeof ring) => ring[k] * (1 - w) + free[k] * w;
  const a = m("ph") + i * m("sp"), R = m("R"), fl = m("flat"), rot = m("rot");
  const lx = Math.cos(a) * R + 7 * w * Math.sin(u * 1.1 + i * 1.7), ly = Math.sin(a) * R * fl + 7 * w * Math.cos(u * .9 + i * 2.3);
  return { x: lx * Math.cos(rot) - ly * Math.sin(rot), y: lx * Math.sin(rot) + ly * Math.cos(rot), a, sp: m("sp") };
};

/** The approved orb study, connected to KB's real library and shared input stack. */
export function Ps2Menu(props: Props) {
  const [view, setView] = createSignal<"home" | "browser" | "options" | "info" | "config">("home");
  const [choice, setChoice] = createSignal(0);
  const items = createMemo(() => {
    const out: number[] = [];
    for (let i = Math.max(0, props.focused - REACH); i <= Math.min(props.count - 1, props.focused + REACH); i++) out.push(i);
    return out;
  });
  const current = () => props.entry(props.focused);
  /** The current tab with two either side, for the row under the title. */
  const tabWindow = createMemo(() => { const at = Math.max(0, props.tabs.indexOf(props.tab));
    return [-2, -1, 0, 1, 2].map((o) => ({ id: props.tabs[at + o], o })).filter((t) => t.id !== undefined) as { id: string; o: number }[]; });
  /** Which way the last platform switch went (-1 left, 1 right): the new row and tabs slide in from that side. */
  let lastTab = props.tab;
  const dir = createMemo(() => { const t = props.tab, a = props.tabs.indexOf(lastTab), b = props.tabs.indexOf(t); lastTab = t; return b > a ? 1 : b < a ? -1 : 0; });
  const meta = () => { const g = props.game(props.focused); return [current()?.subtitle, g?.year, g?.developer].filter(Boolean).join(" · "); };
  const played = () => { const g = props.game(props.focused); if (!g) return "";
    return (g.favorite ? "★ Favorite · " : "") + (g.playSeconds > 0 ? `${formatPlayTime(g.playSeconds)} played · ${formatLastPlayed(g.lastPlayed)}` : "Never played"); };
  const OPTIONS = () => [
    [props.launching || starting() ? "Starting…" : "Start game", "Launch the game"],
    ["Information", "Release, developer, play time and description"],
    ["View cover", "Turn the case and read the back"],
    ["Return to browser", "Back to your games"],
  ];
  /** Starting a game: the disc zooms at the screen and the picture fades to black, then the game launches. */
  const [starting, setStarting] = createSignal(false);
  const start = () => {
    if (starting() || props.launching) return;
    if (!props.game(props.focused)) { props.play(); return; } // in Collections it opens the collection instead
    setStarting(true);
    const wait = fx.anim() === "full" ? 1100 : fx.anim() === "reduced" ? 250 : 0;
    setTimeout(() => { props.play(); if (!props.launching) setStarting(false); }, wait);
  };
  createEffect(on(() => props.launching, (now, was) => { if (was && !now) setStarting(false); }, { defer: true }));
  /** System Configuration: KB's TV Mode sound and animation choices, the version, and the full Settings. */
  const ANIM = ["full", "reduced", "off"] as const;
  const configRows = (): [string, string][] => [
    ["Sound", fx.get("soundTv") ? "On" : "Off"],
    ["Volume", `${fx.get("volumeTv")}%`],
    ["Animations", { full: "Full", reduced: "Reduced", off: "Off" }[fx.get("animTv")]],
    ["Version", "KaneegaBell 0.6.0 development"],
    ["All settings…", ""],
  ];
  const configActions = [
    () => fx.set("soundTv", !fx.get("soundTv")),
    () => fx.set("volumeTv", fx.get("volumeTv") >= 100 ? 20 : fx.get("volumeTv") + 20),
    () => fx.set("animTv", ANIM[(ANIM.indexOf(fx.get("animTv")) + 1) % 3]),
    () => {},
    () => props.settings(),
  ];
  const infoRows = () => {
    const g = props.game(props.focused), d = props.detail; if (!g) return [];
    const year = d?.releaseDate ? new Date(d.releaseDate * 1000).getUTCFullYear() : g.year;
    return ([["Platform", current()?.subtitle], ["Released", year], ["Developer", d?.developer ?? g.developer], ["Genre", d?.genres.slice(0, 3).join(", ")],
      ["Play time", g.playSeconds > 0 ? formatPlayTime(g.playSeconds) : "Never played"], ["Last played", g.lastPlayed ? formatLastPlayed(g.lastPlayed) : null]] as [string, string | number | null | undefined][]).filter(([, v]) => v);
  };
  const [now, setNow] = createSignal(new Date());
  createEffect(() => { if (session.running()) return; setNow(new Date()); const t = setInterval(() => setNow(new Date()), 60_000); onCleanup(() => clearInterval(t)); });
  const clock = () => `${now().toLocaleDateString([], { weekday: "short", day: "numeric", month: "numeric" })}  ${now().toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}`;
  const choose = () => { sfx("select"); if (view() === "home") { if (choice()) { setView("config"); setChoice(0); } else setView("browser"); }
    else if (view() === "browser") setView("options");
    else if (view() === "options") [start, () => setView("info"), props.cover, () => setView("browser")][choice()]?.();
    else if (view() === "info") props.more();
    else if (view() === "config") configActions[choice()]?.(); };
  const back = () => { const was = view(); if (was !== "home") sfx("back"); setView(was === "info" ? "options" : was === "options" ? "browser" : "home"); setChoice(was === "info" ? 1 : 0); };
  const move = (delta: number) => { const next = Math.max(0, Math.min(props.count - 1, props.focused + delta)); if (next !== props.focused) { props.select(next); sfx("move"); } };
  let canvas!: HTMLCanvasElement;
  /** Extra ring speed after the selection moves; eases back to 0. */
  let boost = 0;
  createEffect(on(() => props.focused, () => { boost = Math.min(3, boost + 1.6); }, { defer: true }));
  onMount(() => {
    let alive=true;
    void preparePs2Audio().then(() => {if(alive) syncPs2Audio(!introPlaying());}).catch(() => {});
    const visibility = () => syncPs2Audio(!introPlaying());
    document.addEventListener("visibilitychange", visibility);
    onCleanup(() => { alive=false; document.removeEventListener("visibilitychange", visibility); syncPs2Audio(false); });
    onCleanup(pushHandler(action => {
      if (["menu", "filter", "toggleTv", "secondary", "favorite", "prevPlatform", "nextPlatform", "prevLetter", "nextLetter"].includes(action)) return false;
      if (starting()) return true;
      if (action === "back") { back(); return true; }
      if (action === "toggleGroups") { setView("config"); setChoice(0); return true; }
      if (action === "select") { choose(); return true; }
      if (action === "left" || action === "right") { if (view() === "browser") move(action === "left" ? -1 : 1); return true; }
      if (action === "up" || action === "down") {
        if (view() === "browser") { setView("options"); setChoice(0); }
        else if (view() === "home" || view() === "options" || view() === "config") { const n = view() === "home" ? 2 : view() === "config" ? 5 : 4; setChoice((choice() + (action === "down" ? 1 : n - 1)) % n); sfx("move"); }
        return true;
      }
      return true;
    }));
    // Same final-ring coordinates and phase as the approved canvas intro; bounded work.
    // KB's own animation level decides (Windows' "Show animations" is ignored, as everywhere in KB).
    // 854x480: the intro's own softened 16:9 resolution, so its last frame and the menu's first frame match.
    const ctx = canvas.getContext("2d", { alpha: false })!;ctx.scale(854/1920,480/1080);
    let frame = 0, time = 8.8, previous = performance.now(), wasIntro = introPlaying(), pull = 0;
    const draw = () => {
      ctx.fillStyle = "#000"; ctx.fillRect(0, 0, 1920, 1080);
      const glow = (x: number, y: number, radius: number, color: string, alpha: number) => {
        ctx.globalAlpha = alpha; const g = ctx.createRadialGradient(x, y, 0, x, y, radius);
        g.addColorStop(0, color); g.addColorStop(1, "transparent"); ctx.fillStyle = g; ctx.fillRect(x-radius, y-radius, radius*2, radius*2);
      };
      ctx.globalCompositeOperation = "screen";
      // When the lights bunch into a comet its tail is longer: the trail's samples are spaced further apart in time.
      const gap = .02 + .04 * (1 - smooth(orbAt(0, time).sp / .35)), keep = 1 - .9 * pull;
      for (let i = 0; i < 8; i++) for (let tail = 9; tail >= 0; tail--) {
        const t = time - tail*gap, o = orbAt(i, t), d = .85 + .15 * Math.sin(o.a), x = 780 + o.x * keep, y = 510 + o.y * keep;
        glow(x,y,38*d,"#37b5ff",(1-tail/10)*d*.3);
        if (!tail) { glow(x,y,19*d,"#37b5ff",.9); glow(x,y,12*d,"#d7f9ff",1); ctx.globalAlpha=1;ctx.fillStyle="#f7ffff";ctx.beginPath();ctx.ellipse(x,y,8*d,6.5*d,0,0,Math.PI*2);ctx.fill(); }
      }
      if (pull > 0) glow(780,510,60+140*pull,"#d7f9ff",pull*.9);
      ctx.globalAlpha=1;ctx.globalCompositeOperation="source-over";
    };
    let timer: ReturnType<typeof setTimeout> | undefined, painted=-1;
    const animate = (now: number) => {
      const playing=introPlaying();if(wasIntro&&!playing)time=8.8;wasIntro=playing;
      const moving=fx.anim()==='full'&&!document.hidden&&!session.running()&&!playing;
      if(moving){const step=Math.min(now-previous,100)/1000;
        // Moving the selection gives the ring a brief push that eases off; starting a game pulls the orbs into one bright point.
        boost*=Math.exp(-step*2.2); pull=Math.max(0,Math.min(1,pull+(props.launching||starting()?step*1.4:-step*2.5)));
        time+=step*(1+boost);}
      if(!document.hidden&&!session.running()&&!playing&&painted!==time){draw();painted=time;}
      previous=now;
      if(moving)frame=requestAnimationFrame(animate);else timer=setTimeout(()=>animate(performance.now()),100);
    };
    draw();frame=requestAnimationFrame(animate);onCleanup(()=>{cancelAnimationFrame(frame);clearTimeout(timer);});
  });
  createEffect(() => { fx.soundOn(); fx.volume(); session.running(); syncPs2Audio(!introPlaying()); });
  return <div class="ps2-runtime" classList={{ starting: starting() }} data-view={view()}>
    <canvas ref={canvas} class="ps2-ring" width="854" height="480" aria-hidden="true" />
    <Show when={view() === "home"}><section class="ps2-home"><For each={["Browser", "System Configuration"]}>{(label,i) => <button classList={{active:choice()===i()}} onClick={() => { setChoice(i()); choose(); }}>{label}</button>}</For></section></Show>
    <div class="ps2-clock" aria-hidden="true">{clock()}</div>
    <Show when={view() === "browser" || view() === "options" || view() === "info"}>
      <section class="ps2-browser">
        <header><h1>Browser</h1></header>
        <Show when={props.tab} keyed>{(_tab) => <>
        <nav class="ps2-tabs" style={{ "--dir": dir() }} aria-label="Platforms"><For each={tabWindow()}>{(t) =>
          <button classList={{ active: t.id === props.tab }} style={{ "--d": Math.abs(t.o), "grid-column": t.o + 3 }} onClick={() => { if (t.id !== props.tab) { props.chooseTab(t.id); sfx("move"); } }}>{props.tabLabel(t.id)}</button>
        }</For></nav>
        <div class="ps2-shelf" style={{ "--dir": dir() }} aria-label="Game library">
          <For each={items()}>{i => <Show when={props.entry(i)} keyed>{entry =>
            <button class="ps2-game" classList={{active:i===props.focused}} style={{ "--o": i - props.focused }} aria-label={entry.title} aria-pressed={i===props.focused}
              onClick={() => { if(i===props.focused) {setView("options");setChoice(0);sfx("select");} else {props.select(i);sfx("move");} }}>
              <Object3d game={props.game(i)} entry={entry} /><span class="ps2-label">{entry.title}</span>
            </button>}</Show>}</For>
          <Show when={props.count>0 && props.focused<props.count-1}><span class="ps2-next" aria-hidden="true" /></Show>
        </div>
        </>}</Show>
        <Show when={`${props.focused}`} keyed>{(_selected) =>
          <section class="ps2-copy" aria-live="polite"><h2>{current()?.title ?? "No games here"}</h2><p>{meta()}</p><Show when={played()}><p class="ps2-played">{played()}</p></Show>
            <Show when={props.detail?.description}><p class="ps2-desc">{props.detail!.description}</p></Show>
            <button onClick={() => {setView("options");setChoice(0);sfx("select");}} disabled={!props.count}>Game options</button>
            <Show when={props.error}><p class="warn">{props.error}</p></Show></section>
        }</Show>
        <Show when={props.count > 0}><div class="ps2-track" aria-hidden="true" style={{ "--p": props.count > 1 ? props.focused / (props.count - 1) : 0 }}><i /><small>{props.focused + 1} / {props.count}</small></div></Show>
      </section>
    </Show>
    <Show when={view() === "options"}><section class="ps2-options">
      <h2>{current()?.title}</h2><p class="ps2-meta">{meta()}</p><Show when={played()}><p class="ps2-played">{played()}</p></Show>
      <div class="ps2-option-list"><For each={OPTIONS()}>{([label, hint], i) =>
        <button classList={{active:choice()===i()}} disabled={(props.launching||starting())&&i()===0} onMouseEnter={() => setChoice(i())} onClick={()=>{setChoice(i());choose();}}><span>{label}</span><small>{hint}</small></button>
      }</For></div>
    </section></Show>
    <Show when={view() === "info"}><section class="ps2-info">
      <h2>{current()?.title}</h2>
      <dl><For each={infoRows()}>{([k, v]) => <div><dt>{k}</dt><dd>{v}</dd></div>}</For></dl>
      <Show when={props.detail?.description}><p class="ps2-desc">{props.detail!.description}</p></Show>
      <button onClick={props.more}>Full details</button>
    </section></Show>
    <Show when={view() === "config"}><section class="ps2-options ps2-config">
      <h2>System Configuration</h2>
      <div class="ps2-option-list"><For each={configRows()}>{([label, value], i) =>
        <button classList={{active:choice()===i()}} onMouseEnter={() => setChoice(i())} onClick={() => { setChoice(i()); choose(); }}><span>{label}</span><em>{value}</em></button>
      }</For></div>
    </section></Show>
  </div>;
}
