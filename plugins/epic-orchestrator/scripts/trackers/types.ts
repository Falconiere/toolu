/** Tracker-neutral shapes: an epic lives in GitHub, Jira, or Linear, while
 * the code, branches, and PRs always live in GitHub repos. */

export type TrackerKind = "github" | "jira" | "linear";

export type PrNode = { number: number; state: string; url: string; headRefName: string };

export type EpicInfo = {
  ref: string;
  title: string;
  state: "open" | "closed";
  url: string;
};

/** One child work item, normalized. `ref` is canonical per tracker:
 * `owner/repo#N` (GitHub), `ABC-12` (Jira), `ENG-12` (Linear). */
export type TrackedIssue = {
  ref: string;
  title: string;
  state: "open" | "closed";
  url: string;
  /** GitHub repo that holds the code and the PR. */
  repo: string;
  /** GitHub issue number; null for Jira/Linear items. */
  number: number | null;
  blockers: Record<string, string>;
  prs: PrNode[];
  deps_source: string;
  labels: string[];
  /** Leading slice of the description, for complexity routing. */
  excerpt: string;
};

export interface Tracker {
  readonly kind: TrackerKind;
  /** Canonical epic ref, e.g. `owner/repo#N`, `ABC-12`, `project:<id>`. */
  readonly epicRef: string;
  epic(): Promise<EpicInfo>;
  children(): Promise<TrackedIssue[]>;
  /** Mark a delivered child done. Returns how it ended up closed. */
  closeIssue(ref: string, note: string): Promise<string>;
  /** Tick the child in the epic body checklist, where the tracker has one. */
  tickEpic(ref: string, title: string): Promise<boolean>;
  /** Comment the summary on the epic and close it. */
  closeEpic(summary: string): Promise<string>;
  /** Directory name for this epic's state. */
  stateSlug(): string;
}

export const EXCERPT_CHARS = 1500;

export function excerpt(text: string | null | undefined): string {
  return (text ?? "").trim().slice(0, EXCERPT_CHARS);
}

const REPO_LABEL = /^repo:([\w.-]+\/[\w.-]+)$/i;
const REPO_LINE = /^\s*repo:\s*([\w.-]+\/[\w.-]+)\s*$/im;

/** Code repo for a Jira/Linear item: a `repo:owner/name` label, then a
 * `Repo: owner/name` line in the description, then the run's default. */
export function repoFor(labels: string[], body: string, fallback: string): string {
  for (const l of labels) {
    const m = REPO_LABEL.exec(l.trim());
    if (m?.[1]) return m[1];
  }
  const line = REPO_LINE.exec(body);
  return line?.[1] ?? fallback;
}
