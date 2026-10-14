declare module "virtual:patch-notes" {
  export interface PatchChange {
    /** "new" | "fixed" | "changed" | "improved" | "" */
    kind: string;
    text: string;
  }
  export interface PatchVersion {
    version: string;
    /** As written in the README ("2026-10-15"), or null. */
    date: string | null;
    changes: PatchChange[];
  }
  const notes: PatchVersion[];
  export default notes;
}
