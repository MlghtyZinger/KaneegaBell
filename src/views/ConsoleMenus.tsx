import { createEffect, createMemo, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { Icon, PlatformIcon } from "../components/Icons";
import { Hint } from "../components/Hints";
import { NavScope } from "../components/NavScope";
import { pushHandler, useGamepadPointer } from "../input/actions";
import { session } from "../state/session";
import { events } from "../api";
import { library, collectionKeyOf } from "../state/library";
import { pointerDelta } from "./consolePointer";
import pointerImage from "../assets/wii-pointer.svg";
import { fx } from "../state/fx";
import { channelWindow, xmbWindow } from "./tvShowcase";
import { sfx } from "../fx/audio";
import { introPlaying } from "../fx/Intro";
import wiiMusic from "../assets/sounds/menu_wii_theme.mp3";
import "../styles/console-menus.css";

export interface ConsoleEntry {
  key: string;
  title: string;
  subtitle: string;
  logo?: string;
  picture?: string;
  banner?: string;
}

interface Props {
  style: "wii" | "ps3";
  count: number;
  focused: number;
  entry: (index: number) => ConsoleEntry | undefined;
  select: (index: number) => void;
  play: () => void;
  more: () => void;
  platforms: () => void;
  styles: () => void;
  settings: () => void;
  search: () => void;
  launching: boolean;
  error: string | null;
}

/** Console screens have their own navigation, rather than rearranging Showcase panels. */
export function ConsoleMenus(props: Props) {
  const [opened, setOpened] = createSignal(false);
  const [options, setOptions] = createSignal(false);
  const [closing, setClosing] = createSignal(false);
  let closeTimer: ReturnType<typeof setTimeout> | undefined;
  const closeChannel = () => {
    if (closing()) return;
    if (fx.anim() !== "full") { setOpened(false); return; }
    setClosing(true);
    closeTimer = setTimeout(() => { setOpened(false); setClosing(false); }, 180);
  };
  onCleanup(() => clearTimeout(closeTimer));
  const tabs = createMemo(() => ["all", ...library.platforms().map((platform) => platform.id)]);
  const category = () => Math.max(0, tabs().indexOf(library.platform()));
  const platformName = () => library.platform() === "all" ? "All platforms" : library.platform() === "favorites" ? "Favorites" : library.platform() === "recent" ? "Recently played" : library.platform() === "collections" || collectionKeyOf(library.platform()) ? "Collections" : library.platforms().find((platform) => platform.id === library.platform())?.name ?? "All platforms";
  const tabName = (id: string) => id === "all" ? "All platforms" : library.platforms().find((platform) => platform.id === id)?.shortName ?? id;
  let surface!: HTMLDivElement;
  let music!: HTMLAudioElement;
  const [dock, setDock] = createSignal(-1);
  const [pointer, setPointer] = createSignal({ x: 0, y: 0, visible: false });
  let pointed: HTMLButtonElement | null = null;
  let lastPointerAt = 0;
  const clearPointer = () => { setPointer((previous) => ({ ...previous, visible: false })); pointed?.removeAttribute("data-pointer-hover"); pointed = null; };
  const hitButton = (x: number, y: number) => {
    const button = document.elementFromPoint(x, y)?.closest<HTMLButtonElement>("button");
    return button && !button.disabled && button.closest(".tv") ? button : null;
  };
  const dockButtons = () => Array.from(surface.querySelectorAll<HTMLButtonElement>(".wii-dock button"));
  const focusDock = (index: number) => { setDock(index); dockButtons()[index]?.focus(); };
  const [clock, setClock] = createSignal(new Date());
  const [failed, setFailed] = createSignal<ReadonlySet<string>>(new Set());
  const current = () => props.entry(props.focused);
  const channels = createMemo(() => channelWindow(props.focused, props.count));
  const items = createMemo(() => xmbWindow(props.focused, props.count));
  const move = (delta: number) => props.select(Math.max(0, Math.min(props.count - 1, props.focused + delta)));
  const chooseCategory = (index: number) => {
    const id = tabs()[Math.max(0, Math.min(tabs().length - 1, index))];
    library.setQuery(""); library.setPlatform(id); setOptions(false);
  };
  createEffect(() => {
    props.style;
    clearTimeout(closeTimer); setClosing(false); setOpened(false); setOptions(false); setDock(-1); clearPointer();
  });
  createEffect(() => {
    current()?.key;
    setOptions(false);
  });
  createEffect(() => {
    if (session.running()) return;
    setClock(new Date());
    const timer = setInterval(() => setClock(new Date()), 60_000);
    onCleanup(() => clearInterval(timer));
  });
  onMount(() => {
    const syncMusic = () => {
      const audible = props.style === "wii" && fx.soundOn() && fx.volume() > 0 && !session.running() && !introPlaying() && !document.hidden;
      music.volume = fx.volume() * .45;
      if (audible && music.paused) void music.play().catch(() => {});
      else if (!audible) music.pause();
    };
    createEffect(syncMusic);
    // Retry blocked autoplay on interaction; no polling or audio decoding at startup.
    document.addEventListener("visibilitychange", syncMusic);
    window.addEventListener("pointerdown", syncMusic);
    window.addEventListener("keydown", syncMusic);
    onCleanup(() => {
      document.removeEventListener("visibilitychange", syncMusic);
      window.removeEventListener("pointerdown", syncMusic);
      window.removeEventListener("keydown", syncMusic);
      music.pause(); music.removeAttribute("src"); music.load();
    });
  });
  onMount(() => {
    // The owner requested audible feedback and visible motion for this update, once.
    // Later sound/motion choices remain respected on subsequent opens.
    if (!fx.get("consoleFeedbackReady")) {
      fx.set("animTv", "full"); fx.set("soundTv", true);
      if (fx.get("volumeTv") === 0) fx.set("volumeTv", 60);
      fx.set("consoleFeedbackReady", true);
    }
    let disposed = false;
    void events.onRightStick(([x, y]) => {
      if (props.style !== "wii" || session.running() || document.hidden || Math.max(Math.abs(x), Math.abs(y)) <= .2) { lastPointerAt = 0; return; }
      const now = performance.now(); const elapsed = lastPointerAt ? now - lastPointerAt : 16; lastPointerAt = now;
      const previous = pointer();
      const rect = surface.getBoundingClientRect();
      const px = Math.max(rect.left + 8, Math.min(rect.right - 8, (previous.visible ? previous.x : rect.left + rect.width / 2) + pointerDelta(x, elapsed, rect.width)));
      const py = Math.max(rect.top + 8, Math.min(rect.bottom - 8, (previous.visible ? previous.y : rect.top + rect.height / 2) + pointerDelta(y, elapsed, rect.width)));
      setPointer({ x: px, y: py, visible: true }); setDock(-1);
      const hit = hitButton(px, py);
      if (hit !== pointed) { pointed?.removeAttribute("data-pointer-hover"); hit?.setAttribute("data-pointer-hover", ""); if (hit) sfx("move"); pointed = hit; }
    }).then((unlisten) => { if (disposed) unlisten(); else onUnlisten = unlisten; }).catch(() => {});
    let onUnlisten: (() => void) | undefined;
    onCleanup(() => { disposed = true; onUnlisten?.(); clearPointer(); });
    onCleanup(useGamepadPointer((action) => {
      if (props.style !== "wii" || !pointer().visible || session.running()) return false;
      if (["up", "down", "left", "right", "back"].includes(action)) { clearPointer(); return false; }
      if (action !== "select" && action !== "toggleGroups") return false;
      const position = pointer(); const hit = hitButton(position.x, position.y);
      if (hit) hit.click();
      return true;
    }));
    surface.addEventListener("pointermove", clearPointer);
    onCleanup(() => surface.removeEventListener("pointermove", clearPointer));
  });
  onMount(() => onCleanup(pushHandler((action) => {
    // Shared overlays and global actions stay with the TV mode handler.
    if (["menu", "filter", "toggleTv", "secondary", "prevPlatform", "nextPlatform", "prevLetter", "nextLetter"].includes(action)) return false;
    if (props.style === "wii") {
      if (opened()) {
        if (action === "back") { closeChannel(); return true; }
        return false; // The channel's NavScope handles Start / Wii Menu / More.
      }
      if (dock() >= 0) {
        if (action === "left" || action === "right") { focusDock(Math.max(0, Math.min(2, dock() + (action === "left" ? -1 : 1)))); return true; }
        if (action === "up" || action === "back") { setDock(-1); (document.activeElement as HTMLElement | null)?.blur(); return true; }
        if (action === "select") { dockButtons()[dock()]?.click(); return true; }
        if (action === "down") return true;
      }
      if (action === "down" && props.focused + 4 >= (Math.floor(props.focused / 12) + 1) * 12) { focusDock(0); return true; }
      if (action === "down" && props.focused + 4 >= props.count) { focusDock(0); return true; }
      if (action === "left" || action === "right") { move(action === "left" ? -1 : 1); return true; }
      if (action === "up" || action === "down") { move(action === "up" ? -4 : 4); return true; }
      if (action === "select") { if (current()) setOpened(true); return true; }
    } else {
      if (action === "left" || action === "right") { chooseCategory(category() + (action === "left" ? -1 : 1)); return true; }
      if (action === "up" || action === "down") { move(action === "up" ? -1 : 1); return true; }
      if (action === "select") { if (current()) props.play(); return true; }
      if (action === "favorite") { if (current()) setOptions(true); return true; }
      if (action === "back") { return false; }
    }
    // Do not rotate an invisible 3D case from either console menu.
    if (["turnLeft", "turnRight", "zoomIn", "zoomOut", "toggleGroups"].includes(action)) return true;
    return false;
  })));
  const logo = (entry: ConsoleEntry) => entry.logo && !failed().has(entry.key) ? entry.logo : undefined;
  const markFailed = (key: string) => setFailed((previous) => new Set(previous).add(key));
  const time = () => clock().toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  const date = () => clock().toLocaleDateString([], { weekday: "short", day: "numeric", month: "numeric" });

  return <div ref={surface} class={`console-menu console-${props.style}`} classList={{ "console-paused": !!session.running() }}>
    <audio ref={music} class="console-menu-music" src={wiiMusic} loop preload="none" />
    <Show when={props.style === "wii"} fallback={<>
      <svg class="xmb-wave" viewBox="0 0 1440 500" preserveAspectRatio="none" aria-hidden="true">
        <defs><linearGradient id="xmb-ribbon" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#b9f4ff" stop-opacity=".04" /><stop offset=".6" stop-color="#b9f4ff" stop-opacity=".18" /><stop offset="1" stop-color="#b9f4ff" stop-opacity="0" /></linearGradient></defs>
        <path fill="url(#xmb-ribbon)" d="M0 90 C400 -20 580 110 870 200 S1250 310 1440 230 L1440 285 C1120 380 1010 280 790 240 S300 70 0 165 Z" />
        <path fill="url(#xmb-ribbon)" d="M0 150 C360 15 590 135 870 220 S1200 330 1440 255 L1440 300 C1150 350 1000 300 800 255 S280 100 0 235 Z" />
        <path fill="none" stroke="#c2f5ff" stroke-opacity=".24" stroke-width="1.5" d="M0 145 C400 15 590 130 870 215 S1210 330 1440 250" />
      </svg>
      <div class="xmb-sparkles" aria-hidden="true"><For each={Array.from({ length: 42 }, (_, i) => i)}>{(i) =>
        <i style={{ left: ((i * 37 + 3) % 100) + "%", top: (32 + (i * 13) % 30) + "%", "--spark-time": (5 + i % 5) + "s", "--spark-delay": (-i * .71) + "s" }} />
      }</For></div>
      <div class="xmb-status">{date()}　{time()}</div>
      <nav class="xmb-categories" aria-label="Consoles"><For each={tabs()}>{(id, index) =>
        <button class="xmb-category" classList={{ active: library.platform() === id }} aria-label={tabName(id)} aria-pressed={library.platform() === id} style={{ transform: `translateX(calc(${index() - category()} * var(--xmb-category-gap)))` }} onClick={() => { chooseCategory(index()); sfx("move"); }}>
          <Show when={id !== "all"} fallback={<Icon name="all" />}><PlatformIcon id={id} /></Show><span>{tabName(id)}</span>
        </button>
      }</For></nav>
      <Show when={library.platform()} keyed>{(_platform) =>
        <div class="xmb-items" aria-label="Games"><For each={items()}>{(index) => <Show when={props.entry(index)}>{(entry) =>
          <button class="xmb-item" classList={{ active: index === props.focused }} style={{ transform: `translateY(${index < props.focused ? -26 : index === props.focused ? 0 : 14 + (index - props.focused - 1) * 7.5}vh)` }} aria-label={entry().title} aria-pressed={index === props.focused} onClick={() => { if (index === props.focused) props.play(); else { props.select(index); sfx("move"); } }}>
            <span class="xmb-thumb"><Show when={entry().banner}><img class="xmb-thumb-scene" src={entry().banner} alt="" decoding="async" /></Show><Show when={logo(entry())} fallback={<span class="xmb-thumb-title">{entry().title}</span>}>{(url) => <img class="xmb-thumb-logo" src={url()} alt="" decoding="async" onError={() => markFailed(entry().key)} />}</Show></span>
            <span class="xmb-item-label">{entry().title}<Show when={index === props.focused}><small>{entry().subtitle}</small></Show></span>
          </button>
        }</Show>}</For></div>
      }</Show>
      <div class="xmb-keys"><Hint action="select" label="Enter" /><Hint action="back" label="Back" /><button onClick={() => setOptions(true)}><Hint action="favorite" label="Options" /></button><button onClick={props.styles}><Hint action="filter" label="Menu style" /></button></div>
    </>}>
      <Show when={Math.floor(props.focused / 12) + 1} keyed>{(_page) => <div class="wii-channels" aria-label="Channels"><For each={channels()}>{(index) => <Show when={props.entry(index)}>{(entry) =>
        <button class="wii-channel" classList={{ active: index === props.focused }} aria-label={entry().title} aria-pressed={index === props.focused} onClick={() => { setDock(-1); props.select(index); setOpened(true); sfx("select"); }}>
          <Show when={entry().banner}><img class="wii-channel-scene" src={entry().banner} alt="" decoding="async" /></Show>
          <Show when={logo(entry())} fallback={<span class="wii-channel-title">{entry().title}</span>}>{(url) => <img class="wii-channel-logo" src={url()} alt={entry().title} decoding="async" onError={() => markFailed(entry().key)} />}</Show>
        </button>
      }</Show>}</For><For each={Array.from({ length: 12 - channels().length })}>{() => <div class="wii-channel wii-channel-empty" aria-hidden="true">Wii</div>}</For></div>}</Show>
      <button class="wii-page wii-page-prev" aria-label="Previous channel page" disabled={props.focused < 12} onClick={() => { move(-12); sfx("move"); }}>◀</button>
      <button class="wii-page wii-page-next" aria-label="Next channel page" disabled={Math.floor(props.focused / 12) >= Math.ceil(props.count / 12) - 1} onClick={() => { move(12); sfx("move"); }}>▶</button>
      <div class="wii-dock"><button class="wii-round" classList={{ "dock-focused": dock() === 0 }} onClick={props.settings} aria-label="Wii options">Wii</button><button class="wii-platforms" classList={{ "dock-focused": dock() === 1 }} onClick={props.platforms} aria-label="Platforms"><Icon name="stack" /></button><div class="wii-clock"><span>{time()}</span><small>{date()}</small></div><button class="wii-round" classList={{ "dock-focused": dock() === 2 }} onClick={props.styles} aria-label="Menu style"><Icon name="tv" /></button></div>
      <Show when={opened() && current()}>{(entry) => <div class="wii-channel-open" classList={{ "wii-channel-closing": closing() }}>
        <div class="wii-channel-banner"><Show when={entry().banner}><img class="wii-banner-scene" src={entry().banner} alt="" decoding="async" /></Show><Show when={logo(entry())} fallback={<h1>{entry().title}</h1>}>{(url) => <img class="wii-banner-logo" src={url()} alt={entry().title} decoding="async" />}</Show><p>{entry().subtitle}</p></div>
        <NavScope class="wii-channel-buttons" modal onBack={closeChannel}><button onClick={closeChannel}>Wii Menu</button><button data-autofocus disabled={props.launching} onClick={props.play}>{props.launching ? "Starting…" : "Start"}</button><button class="wii-more" onClick={props.more}>Game information</button></NavScope>
      </div>}</Show>
    </Show>
    <Show when={props.count === 0}><p class="console-empty">No games here. <button onClick={props.platforms}>Choose a platform</button></p></Show>
    <Show when={options()}><div class="xmb-options-overlay" onClick={(event) => event.target === event.currentTarget && setOptions(false)}><NavScope class="xmb-options" modal onBack={() => setOptions(false)}>
      <p>{current()?.title ?? "Game"}</p><button data-autofocus disabled={!current() || props.launching} onClick={props.play}>{props.launching ? "Starting…" : "Start"}</button><button disabled={!current()} onClick={() => { setOptions(false); props.more(); }}>Information</button><button onClick={() => { setOptions(false); props.platforms(); }}>Platforms</button><button onClick={() => { setOptions(false); props.styles(); }}>Menu style</button><button onClick={() => setOptions(false)}>Back</button>
    </NavScope></div></Show>
    <div class="console-platform-name">{platformName()} · {props.count} games</div>
    <Show when={props.style === "wii" && pointer().visible}><div class="wii-pointer" aria-hidden="true" style={{ transform: `translate3d(${pointer().x}px,${pointer().y}px,0)` }}><img src={pointerImage} alt="" /><small>1</small></div></Show>
    <Show when={props.error}><p class="console-error" role="alert">{props.error}</p></Show>
  </div>;
}
