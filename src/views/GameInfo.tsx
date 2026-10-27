import { launchAnimation, launchDone } from "../fx/LaunchFx";
import { createEffect, createResource, createSignal, For, lazy, on, onCleanup, Show } from "solid-js";
import { api, artUrl, backloggdUrl, errorText, events, type GameDetail, type Series } from "../api";
import { formatLastPlayed, formatPlayTime } from "../state/session";
import { library } from "../state/library";
import { NavScope } from "../components/NavScope";
import { MatchPicker } from "../components/MatchPicker";
import { Badge, Hint } from "../components/Hints";
import { Achievements } from "../components/Achievements";
import { CoverViewer } from "./CoverViewer";
import { collections } from "../state/collections";
import { mainSeries, owned } from "../state/series";
import { LaunchWith } from "../components/EmulatorChoice";
import { caseOf } from "../fx/Model";
import { ManualButton } from "../components/ManualButton";
import { osk } from "../state/osk";
import { Icon } from "../components/Icons";
const ArtworkStudio = lazy(() => import("../components/ArtworkStudio").then((m) => ({ default: m.ArtworkStudio })));

function formatSize(bytes: number): string {
  if (bytes >= 1 << 30) return `${(bytes / (1 << 30)).toFixed(1)} GB`;
  if (bytes >= 1 << 20) return `${(bytes / (1 << 20)).toFixed(0)} MB`;
  return `${Math.max(1, Math.round(bytes / 1024))} KB`;
}

/** "2006 · Clover Studio · Capcom": release year, developer, publisher (when known). */
function metaLine(g: GameDetail): string {
  const m = g.meta;
  const year = m.releaseDate ? new Date(m.releaseDate * 1000).getUTCFullYear() : null;
  const pub = m.publisher && m.publisher !== m.developer ? m.publisher : null;
  const players = m.players && m.players > 1 ? `1–${m.players} players` : null;
  return [year, m.developer, pub, players].filter(Boolean).join(" · ");
}

/** Where the game's info comes from, next to Fix match. */
function matchLine(m: GameDetail["meta"], igdb: boolean): string {
  const lb = {
    ok: `LaunchBox: ${m.title ?? "matched"}`,
    manual: `LaunchBox: ${m.title ?? "matched"} (your pick)`,
    ambiguous: "LaunchBox has several possible matches: pick one",
    notfound: "Not found in LaunchBox",
    none: "Not looked up in LaunchBox yet",
  }[m.lbStatus];
  if (!igdb || m.lbStatus === "ok" || m.lbStatus === "manual") return lb;
  return `${lb} · ${m.igdbId ? `IGDB: matched${m.matchStatus === "manual" ? " (your pick)" : ""}` : "not on IGDB"}`;
}

/** A small stack-of-cases mark for series. */
export function SeriesGlyph() {
  return (
    <svg class="series-glyph" viewBox="0 0 16 16" aria-hidden="true">
      <rect x="5" y="1.5" width="9" height="11" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.4" opacity="0.55" />
      <rect x="2" y="3.5" width="9" height="11" rx="1.5" fill="currentColor" />
    </svg>
  );
}

/**
 * The game's series: its name and the games you have from it, in release
 * order (covers; select one to open it).
 */
function SeriesCard(props: { game: GameDetail; series: Series; onOpen: (id: number) => void }) {
  const s = () => props.series;
  const members = () => owned(s());
  // Other series or franchises it belongs to that you also have more of.
  const others = () => props.game.meta.series.filter((x) => x !== s() && owned(x).length >= 2);
  // The series' emblem: the logo picked for it (Collections → Edit), else the
  // clear logo of its earliest game that has one.
  const emblem = () => {
    const col = collections.byKey().get(s().key);
    const chosen = col && collections.chosenPicture(col);
    if (chosen?.kind === "logo") return chosen.url;
    const first = members().find((m) => (m.logo ?? 0) > 0);
    return first ? artUrl(first.id, first.logo!, "l") : null;
  };
  // Once you've started the series: the earliest game you haven't played yet.
  const nextUp = () => (members().some((m) => m.playSeconds > 0) ? members().find((m) => m.playSeconds === 0)?.id : undefined);
  return (
    <div class="series-card">
      <div class="series-text">
        <div class="series-kicker">
          <SeriesGlyph />
          {s().kind === "series" ? "Series" : "Franchise"}
        </div>
        <Show when={emblem()} fallback={<div class="series-name">{s().name}</div>}>
          {(url) => <img class="series-logo" src={url()} alt={s().name} title={s().name} decoding="async" />}
        </Show>
        <Show when={others().length > 0}>
          <div class="series-also">Also part of {others().map((o) => o.name).join(", ")}</div>
        </Show>
      </div>
      <div class="series-covers">
        <For each={members()}>
          {(m) => (
            <button
              class="series-cover"
              classList={{ current: m.id === props.game.id, played: m.playSeconds > 0, next: m.id === nextUp() }}
              title={m.playSeconds > 0 ? `${m.title} · played` : m.id === nextUp() ? `${m.title} · next to play` : m.title}
              onClick={() => m.id !== props.game.id && props.onOpen(m.id)}
            >
              <Show when={m.art > 0} fallback={<span class="series-cover-name">{m.title}</span>}>
                <img src={artUrl(m.id, m.art, "t")} alt={m.title} decoding="async" />
              </Show>
              <Show when={m.playSeconds > 0}>
                <span class="series-mark played-mark" aria-label="Played">
                  ✓
                </span>
              </Show>
              <Show when={m.id === nextUp()}>
                <span class="series-mark next-mark" aria-label="Next to play">
                  ▶
                </span>
              </Show>
            </button>
          )}
        </For>
      </div>
    </div>
  );
}

/** Quote an argv element the way a Windows command line would show it. */
function showArg(a: string): string {
  return /[\s"]/.test(a) || a === "" ? `"${a.replace(/"/g, '\\"')}"` : a;
}

/**
 * Game details with a Play button: cover, play time, what the scanner found,
 * and the exact command a launch runs.
 */
export function GameInfo(props: { id: number; onClose: () => void }) {
  // The game shown: the one opened, or another picked from its series.
  const [id, setId] = createSignal(props.id);
  const [game, { refetch }] = createResource(() => id(), api.getGame);
  const [refreshing, setRefreshing] = createSignal(false);
  const [studio, setStudio] = createSignal(false);
  const [matching, setMatching] = createSignal(false);
  const [viewing, setViewing] = createSignal(false);
  const row = () => library.games().find((g) => g.id === id());
  const [services] = createResource(() => api.getServices().catch(() => null));
  // The game's RetroAchievements id, once the achievements section knows it.
  const [raGameId, setRaGameId] = createSignal<number | null>(null);
  // Loaded with the game, so the page and all its links appear at once. Usually from KB's cache; a game RA
  // hasn't been asked about yet needs one request, so the page waits 400 ms at most (the link then comes later).
  const [glance] = createResource(
    () => id(),
    (gid) =>
      Promise.race([
        api.achievementGlance(gid).catch(() => null),
        new Promise<null>((done) => setTimeout(() => done(null), 400)),
      ]),
  );
  const open = (url: string) => void api.openUrl(url).catch(() => {});

  // Set when Refresh left a cover the user picked.
  const [kept, setKept] = createSignal(false);
  async function refreshCover() {
    setRefreshing(true);
    setKept(false);
    try {
      if ((await api.refreshArt(id())) === "kept") {
        setRefreshing(false);
        setKept(true);
      }
    } catch {
      setRefreshing(false);
    }
  }
  // The art job reports back when it's done; show the new cover then.
  const unlisten = events.onArtFinished(() => {
    setRefreshing(false);
    void refetch();
  });
  onCleanup(() => void unlisten.then((f) => f()));
  const [argv, { refetch: refetchArgv }] = createResource(
    () => id(),
    (id) => api.previewLaunch(id).then((a) => ({ ok: true as const, a }), (e) => ({ ok: false as const, e: errorText(e) })),
  );
  // The emulators this game can use (shown when there's more than one).
  const [emus, { refetch: refetchEmus }] = createResource(
    () => id(),
    (id) => api.gameEmulators(id).catch(() => null),
  );
  async function setEmulator(emulator: string | null) {
    try {
      await api.setGameEmulator(id(), emulator);
    } catch (e) {
      setLaunchError(errorText(e));
    }
    void refetchEmus();
    void refetchArgv();
  }
  // Overview / Achievements / Game files; L1 and R1 (Q / E) switch.
  const [tab, setTab] = createSignal<"overview" | "achievements" | "files">("overview");
  const tabs = [
    { id: "overview" as const, name: "Overview" },
    { id: "achievements" as const, name: "Achievements" },
    { id: "files" as const, name: "Game files" },
  ];
  const stepTab = (d: 1 | -1) => {
    const i = tabs.findIndex((x) => x.id === tab());
    setTab(tabs[(i + d + tabs.length) % tabs.length].id);
  };
  const [launching, setLaunching] = createSignal(false);
  const [launchError, setLaunchError] = createSignal<string | null>(null);
  const [favorite, setFavorite] = createSignal<boolean | null>(null);
  const isFavorite = () => favorite() ?? library.games().find((g) => g.id === id())?.favorite ?? false;
  // The series' color: the main color of its earliest game's cover.
  const colorSource = () => {
    const g = game();
    const s = g && mainSeries(g.meta);
    return s ? (owned(s).find((m) => m.art > 0)?.id ?? null) : null;
  };
  const [seriesColor] = createResource(colorSource, (gid) => api.coverAccent(gid).catch(() => null));
  const tint = () => (colorSource() != null ? seriesColor() : null);
  // Another game from the series opened in place: forget this one's state.
  createEffect(
    on(
      id,
      () => {
        setFavorite(null);
        setLaunchError(null);
        setRaGameId(null);
        setRefreshing(false);
        setKept(false);
        setTab("overview");
      },
      { defer: true },
    ),
  );

  async function toggleFavorite() {
    const next = !isFavorite();
    setFavorite(next);
    await api.setFavorite(id(), next);
    await library.reload();
  }

  /** The title or the subtitle on the on-screen keyboard (the pencils); scans never undo it. */
  function rename(g: GameDetail, part: "title" | "subtitle") {
    const save = (title: string, variant: string) =>
      void api
        .renameGame(g.id, title, variant)
        .then(() => Promise.all([refetch(), library.reload()]))
        .catch((e) => setLaunchError(errorText(e)));
    if (part === "title")
      return osk.open({
        title: "Rename",
        value: g.title,
        status: () => `From the file: ${g.fileTitle} (leave blank to use it)`,
        onDone: (title) => save(title, g.variant ?? ""),
      });
    osk.open({
      title: "Subtitle",
      value: g.variant ?? "",
      status: () => `From the file: ${g.fileVariant ?? "none"} (leave blank for none)`,
      onDone: (variant) => save(g.title, variant),
    });
  }

  async function play() {
    if (launching()) return;
    setLaunching(true);
    setLaunchError(null);
    try {
      // The case opens (or the cartridge goes in) first; Back there cancels.
      if (await launchAnimation(id())) {
        await api.launchGame(id());
        props.onClose(); // the Now Playing screen takes over
      }
    } catch (e) {
      launchDone();
      setLaunchError(errorText(e));
    } finally {
      setLaunching(false);
    }
  }

  return (
    <div class="overlay" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <NavScope class="panel info-panel" modal onBack={props.onClose} onTab={stepTab} onFavorite={() => void toggleFavorite()}>
        <Show when={!glance.loading && game()} fallback={<p class="dp-loading dim">Loading…</p>}>
          {(g) => (
            <div class="dp-body">
              <aside class="dp-side" style={{ "--series": tint() ?? undefined }}>
                <Show when={g().art > 0} fallback={<div class="dp-cover dp-cover-empty dim">No cover yet</div>}>
                  <img class="dp-cover" src={artUrl(g().id, g().art, "f")} alt={`${g().title} cover`} decoding="async" />
                </Show>
                <button class="btn primary big" data-autofocus disabled={launching()} onClick={() => void play()}>
                  {launching() ? "Starting…" : "▶ Play"}
                </button>
                <div class="dp-pair">
                  <button class="btn" classList={{ "fav-on": isFavorite() }} onClick={() => void toggleFavorite()}>
                    {isFavorite() ? "★ Favorite" : "☆ Favorite"}
                  </button>
                  <button class="btn" disabled={!row()} onClick={() => setViewing(true)}>
                    View cover
                  </button>
                </div>
                <ManualButton gameId={g().id} title={g().title} />

                <p class="dp-played dim">
                  {g().playSeconds > 0
                    ? `${formatPlayTime(g().playSeconds)} played · Last played ${formatLastPlayed(g().lastPlayed)}`
                    : "Never played"}
                </p>
                <Show when={emus() && emus()!.options.length > 1 ? emus() : null}>
                  {(e) => (
                    <div class="dp-launch">
                      <span class="dim">Launch with</span>
                      <LaunchWith emus={e()} onChange={(v) => void setEmulator(v)} />
                    </div>
                  )}
                </Show>
                <Show when={launchError()}>
                  <p class="warn">{launchError()}</p>
                </Show>
                <div class="dp-links">
                  <span class="dp-label">Links</span>
                  <div class="dp-chips">
                    <button class="dp-chip" title="Opens in your browser" onClick={() => open(backloggdUrl(g().meta.title ?? g().title, g().meta.slug))}>
                      {g().meta.slug ? "Backloggd" : "Find on Backloggd"} ↗
                    </button>
                    <Show when={g().meta.slug}>
                      {(slug) => (
                        <button class="dp-chip" title="Opens in your browser" onClick={() => open(`https://www.igdb.com/games/${encodeURIComponent(slug())}`)}>
                          IGDB ↗
                        </button>
                      )}
                    </Show>
                    <Show when={raGameId() ?? glance()?.raGameId}>
                      {(rid) => (
                        <button class="dp-chip" title="Opens in your browser" onClick={() => open(`https://retroachievements.org/game/${rid()}`)}>
                          RetroAchievements ↗
                        </button>
                      )}
                    </Show>
                  </div>
                </div>
                <Show when={mainSeries(g().meta)}>
                  {(s) => <SeriesCard game={g()} series={s()} onOpen={setId} />}
                </Show>
              </aside>

              <div class="dp-main" style={{ "--series": tint() ?? undefined }}>
                <header class="dp-head">
                  <Show when={mainSeries(g().meta)}>
                    {(s) => (
                      <div class="series-eyebrow">
                        <SeriesGlyph />
                        <span>{s().name}</span>
                      </div>
                    )}
                  </Show>
                  <h2 class="dp-name">
                    {g().title}
                    <button class="dp-pencil" title="Rename" aria-label="Rename" onClick={() => rename(g(), "title")}>
                      <Icon name="pencil" />
                    </button>
                  </h2>
                  <p class="dim dp-name dp-sub">
                    {g().variant ?? <span class="dp-sub-add">Add a subtitle</span>}
                    <button class="dp-pencil" title="Edit subtitle" aria-label="Edit subtitle" onClick={() => rename(g(), "subtitle")}>
                      <Icon name="pencil" />
                    </button>
                  </p>
                  <p class="meta-line">{[g().platformId.toUpperCase(), metaLine(g())].filter(Boolean).join(" · ")}</p>
                  <Show when={g().meta.genres.length > 0}>
                    <div class="genre-row">
                      <For each={g().meta.genres}>{(name) => <span class="badge">{name}</span>}</For>
                    </div>
                  </Show>
                </header>

                <div class="dp-tabs" role="tablist">
                  <Badge action="prevPlatform" />
                  <For each={tabs}>
                    {(t) => (
                      <button class="dp-tab" classList={{ on: tab() === t.id }} role="tab" aria-selected={tab() === t.id} onClick={() => setTab(t.id)}>
                        {t.name}
                      </button>
                    )}
                  </For>
                  <Badge action="nextPlatform" />
                </div>

                <section class="dp-page" hidden={tab() !== "overview"}>
                  <Show when={g().meta.description} fallback={<p class="dim">No description yet.</p>}>
                    <p class="dp-desc">{g().meta.description}</p>
                  </Show>
                </section>

                {/* Keyed: a game opened from the series loads its own achievements. Always mounted so the RetroAchievements link is known. */}
                <section class="dp-page" hidden={tab() !== "achievements"}>
                  <Show when={id()} keyed>
                    {(gameId) => <Achievements compact gameId={gameId} onRaGame={setRaGameId} />}
                  </Show>
                </section>

                <Show when={tab() === "files"}>
                  <section class="dp-page">
                    <div class="row tight">
                      <button class="btn small" onClick={() => setStudio(true)}>
                        Artwork…
                      </button>
                      <button class="btn small" disabled={refreshing()} onClick={() => void refreshCover()}>
                        {refreshing() ? "Looking…" : "Refresh"}
                      </button>
                      <button class="btn small" onClick={() => setMatching(true)}>
                        Fix match…
                      </button>
                    </div>
                    <Show when={kept()}>
                      <p class="dim small">You picked this cover, so Refresh keeps it. Artwork… picks another.</p>
                    </Show>
                    <dl class="facts">
                      <Show when={g().artSource}>
                        <dt>Cover</dt>
                        <dd>
                          {({ launchbox: "LaunchBox", libretro: "libretro-thumbnails", gametdb: "GameTDB", igdb: "IGDB", sgdb: "SteamGridDB", local: "the game's icon", file: "your image" } as Record<string, string>)[
                            g().artSource!
                          ] ?? g().artSource}
                        </dd>
                      </Show>
                      <dt>Game info</dt>
                      <dd>{matchLine(g().meta, !!services()?.igdb)}</dd>
                      <Show when={g().region}>
                        <dt>Region</dt>
                        <dd>{g().region}</dd>
                      </Show>
                      <Show when={g().serial}>
                        <dt>Serial</dt>
                        <dd>{g().serial}</dd>
                      </Show>
                      <dt>Size</dt>
                      <dd>{formatSize(g().totalSize)}</dd>
                      <dt>Launch file</dt>
                      <dd class="mono">{g().launchPath}</dd>
                      <Show when={g().files.length > 1}>
                        <dt>Discs</dt>
                        <dd>
                          <For each={g().files}>
                            {([path, disc]) => (
                              <div class="mono">
                                {disc ? `Disc ${disc}: ` : ""}
                                {path}
                              </div>
                            )}
                          </For>
                        </dd>
                      </Show>
                    </dl>
                    <h3>Launch command</h3>
                    <Show when={argv()}>
                      {(r) => {
                        const res = r();
                        return res.ok ? (
                          <>
                            <p class="dim small">Runs with {res.a.emulator}</p>
                            <Show when={res.a.notice}>
                              <p class="warn">{res.a.notice}</p>
                            </Show>
                            <pre class="cmd">{res.a.argv.map(showArg).join(" ")}</pre>
                          </>
                        ) : (
                          <p class="warn">{res.e}</p>
                        );
                      }}
                    </Show>
                  </section>
                </Show>
              </div>
            </div>
          )}
        </Show>
        <footer class="dp-foot">
          <div class="hints-row">
            <Hint action="select" label="Select" />
            <Hint action="favorite" label="Favorite" />
            <Hint action="back" label="Close" />
          </div>
          <button class="btn" onClick={props.onClose}>
            Close
          </button>
        </footer>
      </NavScope>
      <Show when={viewing() && row()}>
        {(g) => <CoverViewer game={g()} platformName={library.platforms().find((p) => p.id === g().platformId)?.name ?? g().platformId} onClose={() => setViewing(false)} />}
      </Show>
      <Show when={matching() && game()}>
        {(g) => (
          <MatchPicker
            gameId={g().id}
            title={g().title}
            igdb={!!services()?.igdb}
            onClose={() => setMatching(false)}
            onChanged={() => {
              void refetch();
              void library.reload();
            }}
          />
        )}
      </Show>
      <Show when={studio() && game()}>
        {(g) => (
          <ArtworkStudio
            gameId={g().id}
            title={g().title}
            cart={caseOf(g().platformId).m === "cart"}
            onClose={() => setStudio(false)}
            onChanged={() => {
              void refetch();
              void library.reload();
            }}
          />
        )}
      </Show>
    </div>
  );
}
