/** Shared fixtures of the plan-ledger cases (#262). */
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import {
  BASE,
  commitFile,
  diffShaIn,
  FEATURE,
  featureRepo,
  writeIn,
} from "./pre-tool-modules-c-cases.ts";

export const LEDGER = ".claude/tmp/plan-ledger/feat_example.json";

export type Step = Record<string, unknown>;

/** Isolates plan-ledger: push-review and docs-sync also fire on `git push`. */
export function cfg(top: object = {}, gates: object = { preset: "strict" }): object {
  return {
    version: 1,
    ...top,
    gates: { ...gates, pushReview: { mode: "off" }, docsSync: { mode: "off" } },
  };
}

/** The feature branch, plus a committed code file unless `code` is false. Returns the diff sha. */
export function seed(sb: Sandbox, code = true): string {
  featureRepo(sb);
  if (code) commitFile(sb, "script.sh", "echo hi");
  return diffShaIn(sb.project);
}

export function step(
  id: string,
  title: string,
  status: string,
  sha: string | null,
  extra: Step = {},
) {
  return { id, title, check: "true", status, exit_code: 0, diff_sha: sha, ...extra };
}

/** A ledger for the current branch at `LEDGER`. */
export function put(
  sb: Sandbox,
  steps: Step[],
  extra: Step = {},
  version = 1,
  slugFile = LEDGER,
): void {
  const ledger = {
    version,
    branch: FEATURE,
    base_branch: BASE,
    plan_doc: "docs/toolu/plans/2026-06-14-planning-hardness.md",
    updated_at: "2026-06-14T12:00:00Z",
    summary: { total: steps.length },
    ...extra,
    steps,
  };
  writeIn(sb.project, slugFile, JSON.stringify(ledger));
}
