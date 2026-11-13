import { events } from "../api";
import { activity } from "./activity";
import { countText } from "./activityModel";
export const JOB_TITLES: Record<string, string> = { scan: "Scanning library", art: "Covers", icons: "Creating game icons", lb: "Game info and artwork", wd: "Game series", meta: "Game details", sgdb: "Backgrounds, logos and covers", ra: "Achievements", manuals: "Finding manuals" };
const progress = (key: string, p: { done: number; total: number }, detail = countText(p.done, p.total)) => activity.update(key, JOB_TITLES[key], { status: "running", done: p.done, total: p.total, detail, worker: key !== "manuals" });
const finish = (key: string, s: { cancelled?: boolean; failed?: number }, detail: string) => activity.finish(key, JOB_TITLES[key], s.cancelled ? "Stopped; completed work kept" : detail, s.cancelled ? "cancelled" : s.failed ? "warning" : "done");
const fail = (key: string, message: string) => activity.finish(key, JOB_TITLES[key], message, "failed");
export function listenForActivity() {
  const listeners = [
    events.onLibraryQueue((q) => activity.queue(q.jobs, JOB_TITLES, q.paused, q.cancelled)),
    events.onLibraryWorkerFailed((message) => { for (const j of activity.jobs().filter((j) => j.worker && ["running", "queued"].includes(j.status))) fail(j.key, message); }),
    events.onScanProgress((p) => { if (p.phase !== "done") progress("scan", { done: p.done + (p.filesTotal > 0 ? Math.min(1, p.filesDone / p.filesTotal) : 0), total: p.total }, `${p.source || "Library"}${p.filesTotal > 0 ? ` - ${countText(p.filesDone, p.filesTotal, "files")}` : p.total > 0 ? ` - ${countText(p.done, p.total, "items")}` : " - discovering files..."}`); }),
    events.onScanFinished((s) => finish("scan", { ...s, failed: s.warnings.length }, `${s.added} added - ${s.updated} updated${s.warnings.length ? ` - ${s.warnings.join("; ")}` : ""}`)),
    events.onScanFailed((s) => fail("scan", s)),
    events.onArtProgress((p) => progress("art", p, `${countText(p.done, p.total)} - ${p.found} covers found`)),
    events.onArtFinished((s) => finish("art", s, `${s.found} found - ${s.notFound} unavailable - ${s.failed} failed`)),
    events.onArtFailed((s) => fail("art", s)),
    events.onIconProgress((p) => progress("icons", p, `${countText(p.done, p.total)} - ${p.written} icons created${p.failed ? ` - ${p.failed} failed` : ""}`)),
    events.onIconFinished((s) => finish("icons", s, `${s.written} icons created - ${s.failed} failed`)),
    events.onIconFailed((s) => fail("icons", s)),
    events.onLbProgress((p) => {
      const phase = { checking: "Checking metadata", downloading: "Downloading metadata list", reading: "Reading saved metadata", matching: "Matching games", images: "Fetching artwork" }[p.phase];
      const count = p.phase === "downloading" ? p.total > 0 ? `${(p.done / 1e6).toFixed(1)} / ${(p.total / 1e6).toFixed(1)} MB` : `${(p.done / 1e6).toFixed(1)} MB received - size unknown` : countText(p.done, p.total);
      progress("lb", p, `${phase} - ${count}`);
    }),
    events.onLbFinished((s) => finish("lb", s, `${s.matched} matched - ${s.heroes} backgrounds - ${s.logos} logos`)),
    events.onLbFailed((s) => fail("lb", s)),
    events.onWdProgress((p) => progress("wd", p, `${p.platform || "Platforms"} - ${p.step === "games" ? "Downloading game list" : "Looking up series"} - ${countText(p.done, p.total, "platforms")}`)),
    events.onWdFinished((s) => finish("wd", s, `${s.withSeries} games with series`)),
    events.onWdFailed((s) => fail("wd", s)),
    events.onMetaProgress((p) => progress("meta", p)),
    events.onMetaFinished((s) => finish("meta", s, `${s.matched} matched - ${s.notFound} unavailable - ${s.failed} failed`)),
    events.onMetaFailed((s) => fail("meta", s)),
    events.onSgdbProgress((p) => progress("sgdb", p)),
    events.onSgdbFinished((s) => finish("sgdb", s, `${s.covers} covers - ${s.heroes} backgrounds - ${s.logos} logos - ${s.failed} failed`)),
    events.onSgdbFailed((s) => fail("sgdb", s)),
    events.onRaProgress((p) => progress("ra", p)),
    events.onRaFinished((s) => finish("ra", s, `${s.hashed} checked - ${s.matched} matched`)),
    events.onRaFailed((s) => fail("ra", s)),
    events.onManualMatchProgress((p) => progress("manuals", p)),
    events.onManualMatchFinished((s) => finish("manuals", s, `${s.found} manuals matched`)),
    events.onManualMatchFailed((s) => fail("manuals", s)),
    events.onUpdateProgress((p) => activity.update("update", "KB update", { done: p.downloaded, total: p.total ?? 0, detail: p.total ? `${(p.downloaded / 1e6).toFixed(1)} / ${(p.total / 1e6).toFixed(1)} MB` : "Downloading update - size unknown" })),
  ];
  // Register listeners immediately, and clean up even if their async registration finishes late.
  let disposed = false; const stops: (() => void)[] = [];
  for (const listener of listeners) void listener.then((stop) => disposed ? stop() : stops.push(stop));
  return () => { disposed = true; stops.forEach((stop) => stop()); };
}
