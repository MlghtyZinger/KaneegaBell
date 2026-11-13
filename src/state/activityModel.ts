export type ActivityStatus = "queued" | "running" | "done" | "warning" | "cancelled" | "failed";
export interface ActivityJob {
  id: number; key: string; title: string; detail: string; status: ActivityStatus;
  done: number; total: number; worker?: boolean; gameId?: number;
}
/** Unknown totals stay indeterminate; a progress event is not a success event. */
export function progressValue(job: Pick<ActivityJob, "done" | "total" | "status">): number | undefined {
  if (job.status === "done") return 100;
  if (job.status !== "running" || !Number.isFinite(job.total) || job.total <= 0 || !Number.isFinite(job.done)) return undefined;
  return Math.min(100, Math.max(0, job.done / job.total * 100));
}
export function countText(done: number, total: number, unit = "games"): string {
  return total > 0 ? `${Math.max(0, Math.min(done, total)).toLocaleString()} / ${total.toLocaleString()} ${unit}` : "Preparing...";
}
