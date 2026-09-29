/** `write-state.sh` argv: the bash script's flags, messages and exit statuses (#269). */
import { CliExit } from "@toolu/core/cli";

export const TOOL = "write-state.sh";

export type WriteStateArgs = {
  findingsCount: number;
  /** Raw JSON text, parsed when the document is built (a bad value exits 1 there). */
  reviewers: string;
  findings: string;
  repo: string | undefined;
  branch: string | undefined;
  /** Comma-separated override of the auto-computed file list; `undefined` when absent. */
  reviewedFiles: string | undefined;
};

type Raw = Record<"count" | "reviewers" | "findings" | "repo" | "branch", string> & {
  reviewedFiles: string | undefined;
};

const FLAGS: Readonly<Record<string, keyof Raw>> = {
  "--findings-count": "count",
  "--reviewers": "reviewers",
  "--findings": "findings",
  "--repo": "repo",
  "--branch": "branch",
  "--reviewed-files": "reviewedFiles",
};

/** Parse `argv`; a usage error throws CliExit(2) with the bash wording. */
export function parseArgs(argv: readonly string[]): WriteStateArgs {
  const raw: Raw = {
    count: "",
    reviewers: '["toolu-review:review"]',
    findings: "[]",
    repo: "",
    branch: "",
    reviewedFiles: undefined,
  };
  for (let i = 0; i < argv.length; i += 2) {
    const flag = argv[i] ?? "";
    const key = FLAGS[flag];
    if (key === undefined) throw new CliExit(2, `${TOOL}: unknown arg: ${flag}`);
    const value = argv[i + 1];
    // bash `shift 2` on a lone trailing flag never advanced and looped forever.
    if (value === undefined) throw new CliExit(2, `${TOOL}: ${flag} needs a value`);
    raw[key] = value;
  }
  if (raw.count === "") throw new CliExit(2, `${TOOL}: --findings-count required`);
  if (!/^[0-9]+$/.test(raw.count)) {
    throw new CliExit(2, `${TOOL}: --findings-count must be an integer`);
  }
  return {
    findingsCount: Number(raw.count),
    reviewers: raw.reviewers,
    findings: raw.findings,
    repo: raw.repo === "" ? undefined : raw.repo,
    branch: raw.branch === "" ? undefined : raw.branch,
    reviewedFiles: raw.reviewedFiles,
  };
}
