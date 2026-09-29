/** What every check receives: where it runs, what it reads, where it reports. */
import type { Reporter } from "./report.ts";

export type Mode = "repo" | "file";

export type CheckContext<C> = {
  /** Absolute package (or workspace root) directory; every path is relative to it. */
  readonly root: string;
  readonly config: C;
  readonly report: Reporter;
};
