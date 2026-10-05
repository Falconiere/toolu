/**
 * CI path groups (#458): `.github/ci-paths.json` maps each group to its globs,
 * names the release-only files, and maps every gated workflow job to a group.
 */
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";

/** The data file, relative to the repository root. */
export const CI_PATHS_FILE = ".github/ci-paths.json";

/** The synthetic group that is on when any file outside release-only changed. */
export const CHANGED = "changed";

const Globs = z.array(z.string().min(1)).min(1);

const WorkflowSchema = z.strictObject({
  /** The required aggregate job that checks this workflow's gated jobs, if any. */
  aggregate: z.string().min(1).nullable(),
  /** Other job names in this workflow that branch protection requires. */
  required: z.array(z.string().min(1)),
  /** Gated job id → group name (a key of `groups`, or `changed`). */
  jobs: z.record(z.string().min(1), z.string().min(1)),
});

export const CiPathsSchema = z.strictObject({
  groups: z.record(z.string().regex(/^[a-z][a-z0-9_]*$/), Globs),
  runEverything: Globs,
  releaseOnly: z.strictObject({ paths: Globs, versionKeys: z.array(z.string().min(1)).min(1) }),
  workflows: z.record(z.string().regex(/\.ya?ml$/), WorkflowSchema),
});

export type CiPaths = z.infer<typeof CiPathsSchema>;
export type CiWorkflow = z.infer<typeof WorkflowSchema>;

export class CiPathsError extends Error {}

/** Parse the data file at `path`; unreadable or invalid data names the file. */
export function loadCiPaths(path: string): CiPaths {
  let doc: unknown;
  try {
    doc = JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    throw new CiPathsError(`${path} is not readable JSON: ${String(error)}`);
  }
  const parsed = CiPathsSchema.safeParse(doc);
  if (!parsed.success) {
    throw new CiPathsError(`${path} is invalid: ${z.prettifyError(parsed.error)}`);
  }
  return parsed.data;
}

/** The data file of the repository at `root`. */
export function loadRepoCiPaths(root: string): CiPaths {
  return loadCiPaths(join(root, CI_PATHS_FILE));
}

const globCache = new Map<string, Bun.Glob>();

/** Whether `path` (repo-relative, `/`-separated) matches any of `globs`. */
export function matchesAny(globs: readonly string[], path: string): boolean {
  return globs.some((glob) => {
    let compiled = globCache.get(glob);
    if (compiled === undefined) {
      compiled = new Bun.Glob(glob);
      globCache.set(glob, compiled);
    }
    return compiled.match(path);
  });
}

/** The group names a `changes` job outputs, `changed` last. */
export function outputNames(config: CiPaths): string[] {
  return [...Object.keys(config.groups), CHANGED];
}
