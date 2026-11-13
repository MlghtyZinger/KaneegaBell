import { createRoot, createSignal } from "solid-js";
import { progressValue, type ActivityJob, type ActivityStatus } from "./activityModel";
export { progressValue };

export const activity = createRoot(() => {
  const [jobs, setJobs] = createSignal<ActivityJob[]>([]);
  const [open, setOpen] = createSignal(false);
  let seq = 0;
  function update(key: string, title: string, patch: Partial<ActivityJob> = {}) {
    setJobs((all) => {
      const current = all.find((j) => j.key === key && (j.status === "running" || j.status === "queued"));
      const job: ActivityJob = { id: current?.id ?? ++seq, key, title, status: "running", detail: "Preparing...", done: 0, total: 0, ...current, ...patch };
      const updated = current ? all.map((j) => j.id === job.id ? job : j) : [job, ...all];
      return [...updated.filter((j) => j.status === "running" || j.status === "queued"), ...updated.filter((j) => j.status !== "running" && j.status !== "queued").slice(0, 20)];
    });
  }
  function finish(key: string, title: string, detail: string, status: ActivityStatus = "done") {
    update(key, title, { status, detail });
  }
  function queue(keys: string[], titles: Record<string, string>, paused: boolean, cancelled = false) {
    for (const key of keys) {
      if (!jobs().some((j) => j.key === key && j.status === "running"))
        update(key, titles[key] ?? key, { status: "queued", detail: paused ? "Waiting for the game to close" : "Waiting for the current library job" });
    }
    for (const j of jobs().filter((j) => cancelled && j.status === "queued" && j.worker && !keys.includes(j.key)))
      finish(j.key, j.title, "Stopped before starting", "cancelled");
    // Worker ownership also identifies which entries the Stop library jobs control affects.
    setJobs((all) => all.map((j) => keys.includes(j.key) ? { ...j, worker: true } : j));
  }
  async function track<T>(title: string, work: () => Promise<T>, gameId?: number): Promise<T> {
    const key = `request-${++seq}`;
    update(key, title, { gameId, detail: "Working..." });
    try { const result = await work(); finish(key, title, "Completed"); return result; }
    catch (e) { finish(key, title, String(e), "failed"); throw e; }
  }
  return { jobs, open, setOpen, toggle: () => setOpen((v) => !v), update, finish, queue, track,
    clear: () => setJobs((all) => all.filter((j) => j.status === "running" || j.status === "queued")) };
});
