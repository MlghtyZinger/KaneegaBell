import { createEffect, createMemo, For, onCleanup, Show } from "solid-js";
import { activity, progressValue } from "../state/activity";
import { listenForActivity } from "../state/activityEvents";
import { type ActivityJob } from "../state/activityModel";
import { library } from "../state/library";
import { api } from "../api";
import { NavScope } from "./NavScope";
import { focusables, focusEl } from "../input/focus";
import "../styles/activity.css";

const active = (j: ActivityJob) => j.status === "running" || j.status === "queued";
const labels = { running: "Working", queued: "Queued", done: "Completed", warning: "Needs attention", cancelled: "Stopped", failed: "Failed" };
export function ActivityButton(props: { focused?: boolean }) {
  const count = () => activity.jobs().filter(active).length;
  return <button class="btn small activity-button" classList={{ "nav-focus": props.focused }} title="Activity (Ctrl+J)" aria-haspopup="dialog" aria-expanded={activity.open()} onClick={activity.toggle}>
    <svg viewBox="0 0 24 24" width="19" height="19" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true"><path d="M12 3v12m-5-5 5 5 5-5M4 17v4h16v-4" /></svg>
    Activity<Show when={count()}><span class="activity-count">{count()}</span></Show>
  </button>;
}
function Meter(props: { job: ActivityJob }) {
  const value = () => progressValue(props.job);
  return <Show when={value() !== undefined} fallback={<progress class="activity-meter" aria-label={`${props.job.title} progress (total unknown)`} max={100} />}>
    <progress class="activity-meter" aria-label={`${props.job.title} progress`} max={100} value={value() ?? 0} />
  </Show>;
}
function ActivityRow(props: { id: number }) {
  const job = () => activity.jobs().find((j) => j.id === props.id);
  const gameTitle = (id?: number) => id ? library.games().find((g) => g.id === id)?.title : undefined;
  return <Show when={job()}>{(j) => <article class={`activity-job activity-${j().status}`} tabIndex={0}>
    <div class="activity-job-title"><strong>{j().title}</strong><span class="activity-label">{labels[j().status]}{j().status === "running" && progressValue(j()) !== undefined ? ` - ${Math.floor(progressValue(j())!)}%` : ""}</span></div>
    <Show when={gameTitle(j().gameId)}>{(title) => <p class="activity-game">{title()}</p>}</Show>
    <p class="activity-detail">{j().detail}</p>
    <Show when={j().status === "running" || j().status === "done"}><Meter job={j()} /></Show>
    <Show when={j().status === "failed" || j().status === "warning"}><p class="dim small">Check the message above, then retry from the original screen.</p></Show>
  </article>}</Show>;
}
export function ActivityCenter(props: { hidden: boolean }) {
  onCleanup(listenForActivity());
  const running = createMemo(() => activity.jobs().filter(active));
  const current = () => running().find((j) => j.status === "running") ?? running()[0];
  const recent = () => activity.jobs()[0];
  const shown = () => current() ?? recent();
  const ordered = createMemo(() => [...running().filter((j) => j.status === "running"), ...running().filter((j) => j.status === "queued"), ...activity.jobs().filter((j) => !active(j))]);
  const keyboard = (e: KeyboardEvent) => {
    if (e.ctrlKey && !e.altKey && !e.metaKey && e.key.toLowerCase() === "j" && !props.hidden) { e.preventDefault(); e.stopImmediatePropagation(); activity.toggle(); }
  };
  window.addEventListener("keydown", keyboard, true);
  onCleanup(() => window.removeEventListener("keydown", keyboard, true));
  createEffect(() => { if (props.hidden) activity.setOpen(false); });
  const stop = async () => {
    try { await api.cancelScan(); }
    catch (e) { activity.finish("stop-error", "Stop library jobs", String(e), "failed"); }
  };
  return <>
    <Show when={!props.hidden}>
      <div class="activity-statusbar">
        <ActivityButton />
        <span class="activity-announcement" role="status">{current() ? `${current().title}: ${labels[current().status]}` : "Background activity finished"}</span>
        <Show when={shown()} fallback={<span class="dim">No background activity</span>}>{(job) => <button class="activity-status-summary" onClick={() => activity.setOpen(true)}>
          <span>{job().title}</span><small>{labels[job().status]}<Show when={active(job())}> - {job().detail}</Show></small>
          <Show when={job().status === "running"}><Meter job={job()} /></Show>
        </button>}</Show>
        <Show when={running().length > 1}><small class="dim">{running().length - 1} more</small></Show>
      </div>
    </Show>
    <Show when={!props.hidden && activity.open()}>
      <div class="activity-scrim" onClick={(e) => { if (e.target === e.currentTarget) activity.setOpen(false); }}>
        <div class="activity-dialog" role="dialog" aria-modal="true" aria-labelledby="activity-heading" onKeyDown={(e) => {
          if (e.key !== "Tab") return;
          const scope = e.currentTarget.querySelector<HTMLElement>("[data-nav-scope]");
          const list = scope ? focusables(scope) : [];
          if (!list.length) return;
          const index = list.indexOf(document.activeElement as HTMLElement);
          e.preventDefault(); e.stopPropagation();
          focusEl(list[(index + (e.shiftKey ? -1 : 1) + list.length) % list.length]);
        }}>
          <NavScope class="activity-panel" modal onBack={() => activity.setOpen(false)}>
            <header><div><h2 id="activity-heading">Activity</h2><p class="dim">{running().length ? `${running().length} active or queued` : "Recent activity in this session"}</p></div><button class="btn small" data-autofocus onClick={() => activity.setOpen(false)}>Close</button></header>
            <div class="activity-list">
              <Show when={ordered().length} fallback={<p class="activity-empty dim">Scans, downloads and artwork jobs will appear here.</p>}>
                <For each={ordered().map((j) => j.id)}>{(id) => <ActivityRow id={id} />}</For>
              </Show>
            </div>
            <footer><Show when={running().some((j) => j.worker)}><button class="btn" onClick={() => void stop()}>Stop library jobs</button></Show><button class="btn ghost" onClick={activity.clear}>Clear finished</button></footer>
          </NavScope>
        </div>
      </div>
    </Show>
  </>;
}
