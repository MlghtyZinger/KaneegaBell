import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { api, artUrl, errorText, events, manualPageUrl, overlayBackdropUrl, type ManualOpened, type ManualOutlineItem } from "../api";
import { pushHandler, type Action } from "../input/actions";
import { Badge } from "../components/Hints";
import { setMovesMuted } from "../fx/audio";
import { fx } from "../state/fx";
import { library } from "../state/library";
import { manual } from "../state/manual";
import "../styles/manual.css";

// The approved page turn (artifact "KB Manual Page Turn", 2026-10-27): the turning page is N vertical
// slices of the page image, each a little more turned than the one before, so the page curls over the
// spine (always the full bend). The gutter shadow is drawn into each face, so a turning page carries it.
const N = 18;
const BEND = 2;
// shortcut: pages are drawn at most 2,400 px tall (sharp to ~2.5× zoom on a 1080p screen); raise with MAX_ZOOM if needed.
const PAGE_PX = 2400;
const MAX_ZOOM = 4;
const GUTTER_R = "linear-gradient(90deg, rgba(0,0,0,.28), rgba(0,0,0,.08) 3.5%, rgba(0,0,0,0) 8%)";
const GUTTER_L = "linear-gradient(270deg, rgba(0,0,0,.28), rgba(0,0,0,.08) 3.5%, rgba(0,0,0,0) 8%)";

type Pdf = import("pdfjs-dist").PDFDocumentProxy;

/** The contents as one list (sub-chapters marked) for the panel. */
interface TocRow {
  title: string;
  page: number;
  sub: boolean;
}

const ease = (t: number) => (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2);

async function loadPdf(bytes: ArrayBuffer): Promise<Pdf> {
  const pdfjs = await import("pdfjs-dist");
  pdfjs.GlobalWorkerOptions.workerSrc = (await import("pdfjs-dist/build/pdf.worker.min.mjs?url")).default;
  // Display only: no scripts, no eval, nothing fetched from outside the file.
  return pdfjs.getDocument({ data: new Uint8Array(bytes), isEvalSupported: false, enableXfa: false, disableAutoFetch: true, stopAtErrors: false }).promise;
}

/** The PDF's bookmarks with page numbers (counting from 1); empty when it has none. */
async function pdfOutline(doc: Pdf): Promise<ManualOutlineItem[]> {
  const out = (await doc.getOutline().catch(() => null)) ?? [];
  const walk = async (items: typeof out): Promise<ManualOutlineItem[]> => {
    const rows: ManualOutlineItem[] = [];
    for (const it of items) {
      try {
        const dest = typeof it.dest === "string" ? await doc.getDestination(it.dest) : it.dest;
        if (!dest || !dest[0]) continue;
        const page = typeof dest[0] === "number" ? dest[0] + 1 : (await doc.getPageIndex(dest[0])) + 1;
        rows.push({ title: it.title, page, items: it.items?.length ? await walk(it.items) : undefined });
      } catch {
        // A bookmark pointing nowhere is left out.
      }
    }
    return rows;
  };
  return walk(out).catch(() => []);
}

/** A game's manual as a book: two pages side by side, turned like paper. */
export function ManualViewer(props: { gameId: number; title: string; overlay?: boolean; backdrop?: boolean; onClose: () => void }) {
  // Over a game, KB's window shows up only once the game's picture covers it, then the manual fades in;
  // leaving fades back to that picture before KB's window goes (it matches the game behind).
  const [entered, setEntered] = createSignal(!props.overlay);
  const [shot, setShot] = createSignal<string | null>(null);
  const frames = (n: number) => new Promise<void>((r) => (n ? requestAnimationFrame(() => void frames(n - 1).then(r)) : r()));
  let leaving = false;
  const close = () => {
    if (!props.overlay) return props.onClose();
    if (leaving) return;
    leaving = true;
    setEntered(false);
    setTimeout(props.onClose, fx.anim() === "off" ? 0 : 320);
  };
  if (props.overlay) {
    void (async () => {
      if (props.backdrop) {
        // One address for both copies (sharp and blurred), so the picture loads once.
        setShot(overlayBackdropUrl());
        await (document.querySelector(".mv-shot") as HTMLImageElement | null)?.decode().catch(() => {});
      }
      // Shown only once the window is fullscreen and drawn at that size (else its first frame is blank).
      for (let i = 0; i < 40 && (Math.abs(innerWidth - screen.width) > 2 || Math.abs(innerHeight - screen.height) > 2); i++) await frames(1);
      await frames(3);
      await api.manualOverlayShown().catch(() => {});
      await frames(1);
      setEntered(true);
    })();
    let unlisten: (() => void) | undefined;
    void events.onManualOverlayLeave(close).then((u) => (alive ? (unlisten = u) : u()));
    onCleanup(() => unlisten?.());
  }
  const cover = () => library.games().find((g) => g.id === props.gameId);
  const bgUrl = () => {
    const g = cover();
    return g?.hero ? artUrl(props.gameId, g.hero, "h") : g?.art ? artUrl(props.gameId, g.art, "f") : null;
  };
  const [phase, setPhase] = createSignal<"loading" | "ready" | "error">("loading");
  const [error, setError] = createSignal("");
  const [info, setInfo] = createSignal<ManualOpened | null>(null);
  const [count, setCount] = createSignal(0);
  const [spread, setSpread] = createSignal(0);
  const [toc, setToc] = createSignal<TocRow[]>([]);
  const [tocOpen, setTocOpen] = createSignal(false);
  const [tocAt, setTocAt] = createSignal(0);
  const [zoom, setZoomShown] = createSignal(1);
  const [pan, setPan] = createSignal({ x: 0, y: 0 });
  const [size, setSize] = createSignal({ w: 400, h: 560 });
  const [, setRev] = createSignal(0); // bumps when a page image arrives

  let doc: Pdf | null = null;
  let aspect = 0.72; // page width / height, from the first page
  const urls = new Map<number, string>(); // page -> image (object URLs are revoked on close)
  const made = new Map<number, Promise<string>>();
  const toSave = new Map<number, Blob>(); // downloaded PDF pages still to hand to KB
  const saved = new Set<number>();
  let converting = false;
  let alive = true;
  let busy = false;

  let bookEl!: HTMLDivElement;
  let hl!: HTMLDivElement;
  let hr!: HTMLDivElement;
  let castL!: HTMLDivElement;
  let castR!: HTMLDivElement;
  let leaf!: HTMLDivElement;
  const strips: { s: HTMLDivElement; f: HTMLDivElement; b: HTMLDivElement; fs: HTMLDivElement; bs: HTMLDivElement }[] = [];

  setMovesMuted(true);
  onCleanup(() => {
    alive = false;
    setMovesMuted(false);
    for (const u of urls.values()) if (u.startsWith("blob:")) URL.revokeObjectURL(u);
    void doc?.destroy();
  });

  const last = () => Math.floor(count() / 2);
  const leftOf = (s: number) => (s * 2 - 1 >= 0 && s * 2 - 1 < count() ? s * 2 - 1 : null);
  const rightOf = (s: number) => (s * 2 < count() ? s * 2 : null);

  /** One page's image: a stored page, or drawn now from the PDF / the user's CBZ. */
  function pageUrl(n: number): Promise<string> {
    const have = made.get(n);
    if (have) return have;
    const o = info()!;
    const p = (async () => {
      if (o.kind === "pages") return manualPageUrl(props.gameId, n, o.version);
      if (o.kind === "cbz") return URL.createObjectURL(new Blob([await api.manualPage(props.gameId, n)]));
      const page = await doc!.getPage(n + 1);
      const v1 = page.getViewport({ scale: 1 });
      const scale = Math.min(PAGE_PX / v1.height, (PAGE_PX * 2) / v1.width);
      const vp = page.getViewport({ scale });
      const canvas = document.createElement("canvas");
      canvas.width = Math.round(vp.width);
      canvas.height = Math.round(vp.height);
      // "print" draws in one go; the display intent waits on animation frames, which stall while KB's window is hidden.
      await page.render({ canvasContext: canvas.getContext("2d")!, viewport: vp, intent: "print" }).promise;
      page.cleanup();
      const blob = await new Promise<Blob>((ok, fail) => canvas.toBlob((b) => (b ? ok(b) : fail(new Error("page image"))), "image/webp", 0.82));
      canvas.width = canvas.height = 0;
      if (o.convert && !saved.has(n)) toSave.set(n, blob);
      return URL.createObjectURL(blob);
    })();
    made.set(n, p);
    p.then((u) => {
      urls.set(n, u);
      setRev((r) => r + 1);
      paint();
    }).catch(() => made.delete(n));
    return p;
  }

  const bg = (n: number | null, gutter: string) => (n === null || !urls.get(n) ? "none" : `${gutter}, url("${urls.get(n)}")`);

  /** The pages at rest (and, during a turn, the ones underneath). */
  let under: { l: number | null; r: number | null } = { l: null, r: null };
  function paint() {
    if (!hl) return;
    const { w, h } = size();
    for (const [el, n, g] of [[hl, under.l, GUTTER_L], [hr, under.r, GUTTER_R]] as const) {
      el.style.backgroundImage = bg(n, g);
      el.style.backgroundSize = `${w}px ${h}px, ${w}px ${h}px`;
      el.classList.toggle("blank", n === null);
      el.classList.toggle("waiting", n !== null && !urls.get(n));
    }
  }

  function show(s: number) {
    under = { l: leftOf(s), r: rightOf(s) };
    paint();
    // The pages around it, so the next turn has them ready.
    for (const n of [s * 2 - 3, s * 2 - 2, s * 2 - 1, s * 2, s * 2 + 1, s * 2 + 2]) if (n >= 0 && n < count()) void pageUrl(n);
  }

  function fit() {
    // Over a game the book takes nearly the whole screen.
    const maxW = innerWidth * (props.overlay ? 0.98 : 0.94);
    const maxH = innerHeight * (props.overlay ? 0.88 : 0.8);
    const h = Math.min(maxH, maxW / (2 * aspect));
    setSize({ w: Math.round(h * aspect), h: Math.round(h) });
    paint();
  }

  // ---------------------------------------------------------------- the turn (as in the approved mockup)

  function pose(p: number, dir: number) {
    const { w } = size();
    const sw = w / N;
    const A = p * Math.PI;
    const c = dir * Math.sin(p * Math.PI) * BEND;
    let x = 0;
    let z = 0;
    for (let k = 0; k < N; k++) {
      const th = Math.max(0, Math.min(Math.PI, A + c * ((k + 0.5) / N)));
      const cs = Math.cos(th);
      const st = strips[k];
      st.s.style.transform = `translate3d(${x.toFixed(2)}px,0,${z.toFixed(2)}px) rotateY(${(-th).toFixed(4)}rad)`;
      st.fs.style.opacity = (0.45 * (1 - Math.max(0, cs))).toFixed(3);
      st.bs.style.opacity = (0.45 * (1 - Math.max(0, -cs))).toFixed(3);
      x += sw * cs;
      z += sw * Math.sin(th);
    }
    castR.style.opacity = (Math.sin(p * Math.PI) * (1 - p) * 1.2).toFixed(3);
    castL.style.opacity = (Math.sin(p * Math.PI) * p * 1.2).toFixed(3);
  }

  function dressLeaf(front: number | null, back: number | null) {
    const { w, h } = size();
    const sw = w / N;
    strips.forEach((st, k) => {
      st.s.style.width = `${sw + 1}px`;
      st.f.style.backgroundImage = bg(front, GUTTER_R);
      st.b.style.backgroundImage = bg(back, GUTTER_L);
      st.f.style.backgroundSize = st.b.style.backgroundSize = `${w}px ${h}px, ${w}px ${h}px`;
      st.f.style.backgroundPosition = `${-k * sw}px 0, ${-k * sw}px 0`;
      const off = -(w - k * sw - sw - 1);
      st.b.style.backgroundPosition = `${off}px 0, ${off}px 0`;
      st.f.classList.toggle("blank", front === null);
      st.b.classList.toggle("blank", back === null);
    });
  }

  // A tap during a turn is turned (quickly) right after it. R1 held repeats every ~0.1 s: it keeps
  // turning only while those still arrive, so letting go stops at the page being turned.
  let queued = 0;
  let presses = 0; // during the current turn
  let lastPress = 0;
  function flip(dir: 1 | -1) {
    lastPress = performance.now();
    if (!busy) return void turnTo(spread() + dir);
    presses++;
    queued = queued === -dir ? 0 : dir;
  }
  function settle(s: number) {
    setSpread(s);
    show(s);
    manual.remember(props.gameId, s);
  }

  async function turnTo(target: number, quick = false) {
    if (busy || phase() !== "ready" || target === spread() || target < 0 || target > last()) return;
    const from = spread();
    const dir = Math.sign(target - from);
    const level = fx.anim();
    if (level === "off") {
      settle(target);
      return;
    }
    busy = true;
    const front = dir > 0 ? rightOf(from) : rightOf(target);
    const back = dir > 0 ? leftOf(target) : leftOf(from);
    // The pages it shows: wait briefly for ones not drawn yet, so a turn never shows a blank sheet.
    await Promise.race([Promise.all([front, back, dir > 0 ? rightOf(target) : leftOf(target)].filter((n): n is number => n !== null).map(pageUrl)), new Promise((r) => setTimeout(r, 700))]);
    if (!alive) return;
    dressLeaf(front, back);
    under = dir > 0 ? { l: leftOf(from), r: rightOf(target) } : { l: leftOf(target), r: rightOf(from) };
    paint();
    pose(dir > 0 ? 0 : 1, dir);
    leaf.style.visibility = "visible";
    const d = quick ? (level === "reduced" ? 180 : 300) : level === "reduced" ? 420 : 1000;
    let lastT: number | null = null;
    let t = 0;
    const step = (now: number) => {
      if (!alive) return;
      if (lastT !== null) t += Math.min(now - lastT, 34) / d; // a slow frame slows the turn instead of skipping it
      lastT = now;
      const e = ease(Math.min(t, 1));
      pose(dir > 0 ? e : 1 - e, dir);
      if (t < 1) {
        requestAnimationFrame(step);
        return;
      }
      settle(target);
      leaf.style.visibility = "hidden";
      castL.style.opacity = castR.style.opacity = "0";
      busy = false;
      const next = queued && (presses < 2 || performance.now() - lastPress < 160) ? target + queued : -1;
      queued = presses = 0;
      if (next >= 0 && next <= last()) return void turnTo(next, true);
      void convertSome();
    };
    requestAnimationFrame(step);
  }

  // ---------------------------------------------------------------- shrinking a downloaded manual

  /** Hands drawn pages to KB, then draws the rest one by one between turns; done: the PDF is dropped. */
  async function convertSome() {
    const o = info();
    if (!o?.convert || converting || !alive) return;
    converting = true;
    try {
      for (let n = 0; n < count() && alive; n++) {
        while (busy && alive) await new Promise((r) => setTimeout(r, 200));
        if (saved.has(n)) continue;
        if (!toSave.has(n)) {
          if (made.has(n)) {
            // Drawn before converting started: draw again for its blob.
            made.delete(n);
          }
          await pageUrl(n);
        }
        const blob = toSave.get(n);
        if (!blob) continue;
        await api.manualSavePage(props.gameId, n, new Uint8Array(await blob.arrayBuffer()));
        toSave.delete(n);
        saved.add(n);
        await new Promise((r) => setTimeout(r, 30));
      }
      if (alive && saved.size === count()) {
        const outline = toc().length ? (info()?.outline ?? null) : null;
        await api.manualFinish(props.gameId, count(), outline);
      }
    } catch (e) {
      console.warn("manual pages not kept yet:", errorText(e));
    } finally {
      converting = false;
    }
  }

  // ---------------------------------------------------------------- opening

  onMount(() => {
    addEventListener("resize", fit);
    onCleanup(() => removeEventListener("resize", fit));
    void (async () => {
      try {
        const o = await api.manualOpen(props.gameId);
        if (!alive) return;
        setInfo(o);
        let outline = o.outline ?? [];
        if (o.kind === "pdf") {
          doc = await loadPdf(await api.manualFile(props.gameId));
          setCount(doc.numPages);
          const v = (await doc.getPage(1)).getViewport({ scale: 1 });
          aspect = v.width / v.height;
          outline = await pdfOutline(doc);
          o.outline = outline.length ? outline : null;
        } else {
          setCount(o.pages ?? 0);
          const first = await pageUrl(0);
          aspect = await new Promise<number>((ok) => {
            const img = new Image();
            img.onload = () => ok(img.naturalWidth / img.naturalHeight || 0.72);
            img.onerror = () => ok(0.72);
            img.src = first;
          });
        }
        if (!alive) return;
        if (!count()) throw new Error("This manual has no pages.");
        aspect = Math.min(1.6, Math.max(0.4, aspect));
        setToc(outline.flatMap((i) => [{ title: i.title, page: i.page, sub: false }, ...(i.items ?? []).map((c) => ({ title: c.title, page: c.page, sub: true }))]));
        fit();
        setPhase("ready");
        // Back where it was left (over a game, between visits).
        const start = Math.min(manual.place(props.gameId), last());
        setSpread(start);
        show(start);
        void convertSome();
      } catch (e) {
        if (!alive) return;
        setError(errorText(e));
        setPhase("error");
      }
    })();
  });

  // ---------------------------------------------------------------- zoom and pan (the cover viewer's glide)

  // Zoom and pan chase their targets every frame, so held presses (repeats) blend into one glide;
  // a drag follows the mouse directly.
  let zoomTarget = 1;
  let zoomNow = 1;
  let panTarget = { x: 0, y: 0 };
  let raf = 0;
  let lastFrame = 0;
  const glide = (now: number) => {
    const k = 1 - Math.exp(-Math.min(now - (lastFrame || now - 16), 50) / 90);
    lastFrame = now;
    zoomNow = Math.abs(zoomTarget - zoomNow) < zoomTarget * 0.002 ? zoomTarget : zoomNow + (zoomTarget - zoomNow) * k;
    setZoomShown(zoomNow);
    const p = pan();
    const settled = Math.abs(panTarget.x - p.x) < 0.5 && Math.abs(panTarget.y - p.y) < 0.5;
    setPan(settled ? panTarget : { x: p.x + (panTarget.x - p.x) * k, y: p.y + (panTarget.y - p.y) * k });
    raf = zoomNow === zoomTarget && settled ? 0 : requestAnimationFrame(glide);
    if (!raf) lastFrame = 0;
  };
  const start = () => {
    if (!raf) raf = requestAnimationFrame(glide);
  };
  const zoomTo = (z: number) => {
    zoomTarget = Math.min(MAX_ZOOM, Math.max(1, z));
    start();
  };
  onCleanup(() => cancelAnimationFrame(raf));
  const movePan = (dx: number, dy: number) => {
    panTarget = { x: panTarget.x + dx * innerWidth * 0.06, y: panTarget.y + dy * innerHeight * 0.06 };
    start();
  };
  const reset = () => {
    panTarget = { x: 0, y: 0 };
    zoomTo(1);
  };

  // ---------------------------------------------------------------- controls

  const jump = (row: TocRow) => {
    setTocOpen(false);
    void turnTo(Math.floor(row.page / 2));
  };
  const openToc = () => {
    if (!toc().length) return;
    // The chapter being read is marked.
    const here = spread() * 2 + 1;
    let at = 0;
    toc().forEach((r, i) => r.page <= here && (at = i));
    setTocAt(at);
    setTocOpen(true);
  };

  onCleanup(
    pushHandler((a: Action) => {
      if (tocOpen()) {
        if (a === "back" || a === "favorite") setTocOpen(false);
        else if (a === "up") setTocAt((i) => Math.max(0, i - 1));
        else if (a === "down") setTocAt((i) => Math.min(toc().length - 1, i + 1));
        else if (a === "select") jump(toc()[tocAt()]);
        return true;
      }
      // The cover viewer's controls: ○ closes, ✕ resets, L2/R2 zoom, D-pad / left stick move, the right
      // stick turns (here: the page); L1/R1 turn pages too, △ opens Contents.
      switch (a) {
        case "back": close(); break;
        case "select": reset(); break;
        case "favorite": openToc(); break;
        case "prevPlatform": case "turnLeft": flip(-1); break;
        case "nextPlatform": case "turnRight": flip(1); break;
        case "left": movePan(1, 0); break;
        case "right": movePan(-1, 0); break;
        case "up": movePan(0, 1); break;
        case "down": movePan(0, -1); break;
        case "prevLetter": zoomTo(zoomTarget / 1.25); break;
        case "nextLetter": zoomTo(zoomTarget * 1.25); break;
      }
      return true; // modal
    }),
  );

  // Mouse: the wheel zooms; dragging moves the book; a click on a page (no drag) turns it.
  let drag: { x: number; y: number; px: number; py: number; moved: boolean } | null = null;
  const onDown = (e: PointerEvent) => {
    if ((e.target as HTMLElement).closest(".mv-chips, .mv-toc")) return;
    drag = { x: e.clientX, y: e.clientY, px: pan().x, py: pan().y, moved: false };
  };
  const onMove = (e: PointerEvent) => {
    if (!drag) return;
    if (Math.abs(e.clientX - drag.x) + Math.abs(e.clientY - drag.y) > 4) drag.moved = true;
    if (!drag.moved) return;
    panTarget = { x: drag.px + e.clientX - drag.x, y: drag.py + e.clientY - drag.y };
    setPan(panTarget);
  };
  const onUp = (e: PointerEvent) => {
    const d = drag;
    drag = null;
    if (!d || d.moved || !bookEl || (e.target as HTMLElement).closest(".mv-chips, .mv-toc")) return;
    const r = bookEl.getBoundingClientRect();
    if (e.clientY < r.top || e.clientY > r.bottom) return;
    flip(e.clientX > r.left + r.width / 2 ? 1 : -1);
  };

  const pagesLabel = () => {
    const l = leftOf(spread());
    const r = rightOf(spread());
    return [l, r].filter((n): n is number => n !== null).map((n) => n + 1).join("–") + ` of ${count()}`;
  };

  return (
    <div
      class="mv"
      classList={{ "mv-over-game": props.overlay, "mv-in": entered() }}
      onWheel={(e) => zoomTo(zoomTarget * (e.deltaY < 0 ? 1.15 : 1 / 1.15))}
      onPointerDown={onDown}
      onPointerMove={onMove}
      onPointerUp={onUp}
    >
      {/* Over a game: the game's screen as it was when the manual came up, first sharp (KB appears without a
          seam), then softly blurred behind the book; without that picture, the game's cover, more blurred. */}
      <Show when={props.overlay && shot()}>{(src) => <img class="mv-shot" src={src()} alt="" />}</Show>
      {/* Otherwise (and over a game with no picture): the game's background, else its cover, more blurred. */}
      <Show when={props.backdrop ? shot() : bgUrl()}>
        {(src) => <img class="mv-bg" classList={{ cover: !props.backdrop && !cover()?.hero }} src={src()} alt="" decoding="async" />}
      </Show>
      <header class="mv-head">
        <span class="mv-tag">Manual</span>
        <span class="mv-title">{props.title}</span>
        <Show when={info()}>{(o) => <span class="mv-source">From {o().source}</span>}</Show>
      </header>

      <Show when={phase() === "loading"}>
        <p class="mv-msg">Getting the manual ready… The first time, it downloads (big manuals take a minute).</p>
      </Show>
      <Show when={phase() === "error"}>
        <div class="mv-msg mv-error">
          <p>{error()}</p>
          <button class="btn" onClick={close}>Close<Badge action="back" /></button>
        </div>
      </Show>

      <div class="mv-stage" style={{ visibility: phase() === "ready" ? "visible" : "hidden" }}>
        <div class="mv-pan" style={{ transform: `translate(${pan().x}px, ${pan().y}px)` }}>
        <div class="mv-zoom" style={{ transform: `scale(${zoom()})` }}>
          <div class="mv-book" ref={bookEl} style={{ width: `${size().w * 2}px`, height: `${size().h}px` }}>
            <div class="mv-half l" ref={hl} style={{ width: `${size().w}px` }}><div class="mv-cast" ref={castL} /></div>
            <div class="mv-half r" ref={hr} style={{ left: `${size().w}px`, width: `${size().w}px` }}><div class="mv-cast" ref={castR} /></div>
            <div class="mv-leaf" ref={leaf} style={{ left: `${size().w}px`, width: `${size().w}px` }}>
              <For each={Array.from({ length: N })}>
                {() => {
                  let s!: HTMLDivElement, f!: HTMLDivElement, b!: HTMLDivElement, fs!: HTMLDivElement, bs!: HTMLDivElement;
                  onMount(() => strips.push({ s, f, b, fs, bs }));
                  return (
                    <div class="mv-strip" ref={s}>
                      <div class="mv-face" ref={f}><div class="mv-shade" ref={fs} /></div>
                      <div class="mv-face mv-back" ref={b}><div class="mv-shade" ref={bs} /></div>
                    </div>
                  );
                }}
              </For>
            </div>
          </div>
        </div>
        </div>
      </div>

      <Show when={phase() === "ready"}>
        <div class="mv-chips">
          <Show when={toc().length}>
            <button onClick={() => (tocOpen() ? setTocOpen(false) : openToc())}>Contents<Badge action="favorite" /></button>
          </Show>
          <button onClick={() => flip(-1)} disabled={spread() === 0} aria-label="Previous pages"><Badge action="prevPlatform" />‹</button>
          <span>{pagesLabel()}</span>
          <button onClick={() => flip(1)} disabled={spread() >= last()} aria-label="Next pages">›<Badge action="nextPlatform" /></button>
          <button onClick={() => zoomTo(zoomTarget / 1.25)} aria-label="Zoom out"><Badge action="prevLetter" />−</button>
          <span>{Math.round(zoom() * 100)}%</span>
          <button onClick={() => zoomTo(zoomTarget * 1.25)} aria-label="Zoom in">+<Badge action="nextLetter" /></button>
          <button onClick={reset}>Reset<Badge action="select" /></button>
          <button onClick={close}>Close<Badge action="back" /></button>
        </div>
      </Show>

      <Show when={tocOpen()}>
        <nav class="mv-toc" aria-label="Contents">
          <h2>Contents</h2>
          <ul>
            <For each={toc()}>
              {(row, i) => (
                <li>
                  <button classList={{ sub: row.sub, here: i() === tocAt() }} onClick={() => jump(row)} onMouseEnter={() => setTocAt(i())}>
                    <span>{row.title}</span>
                    <span class="pg">{row.page}</span>
                  </button>
                </li>
              )}
            </For>
          </ul>
        </nav>
      </Show>
    </div>
  );
}
