/**
 * Every check id, in run order. A check id is the unit of ownership:
 * `ownedByLinter` names ids and skips the WHOLE check, so no id straddles what
 * a linter can see and what it cannot. File-addressable checks run first so
 * --file mode stays cheap; `patterns` is batched (one ast-grep for all paths).
 */
import { bannedDeps } from "./checks/banned-deps.ts";
import { colocatedTests } from "./checks/colocated-tests.ts";
import { fileSize } from "./checks/file-size.ts";
import { filenameCase } from "./checks/filename-case.ts";
import { folderReadmes } from "./checks/folder-readmes.ts";
import { folderTree } from "./checks/folder-tree.ts";
import { lintSuppressions } from "./checks/lint-suppressions.ts";
import { noBarrels } from "./checks/no-barrels.ts";
import { requiredFiles } from "./checks/required-files.ts";
import { secretContent } from "./checks/secret-content.ts";
import { secrets } from "./checks/secrets.ts";
import { shadowConfigs } from "./checks/shadow-configs.ts";
import { testTree } from "./checks/test-tree.ts";
import type { GuardrailsConfig, RepoFacts } from "./config.ts";
import type { CheckContext, Mode } from "./context.ts";

type FileCheck = (ctx: CheckContext<GuardrailsConfig>, mode: Mode, path: string) => void;
type RepoCheck = (ctx: CheckContext<RepoFacts>, mode: Mode) => void;

export const FILE_CHECKS: ReadonlyArray<readonly [string, FileCheck]> = [
  ["folder-tree", folderTree],
  ["file-size", fileSize],
  ["colocated-tests", colocatedTests],
  ["no-barrels", noBarrels],
  ["filename-case", filenameCase],
  ["secret-content", secretContent],
  ["lint-suppressions", lintSuppressions],
];

/** Repo-scope checks that walk the source tree. */
export const TREE_CHECKS: ReadonlyArray<readonly [string, FileCheck]> = [
  ["folder-readmes", (ctx, mode) => folderReadmes(ctx, mode)],
  ["test-tree", (ctx, mode) => testTree(ctx, mode)],
];

/** Repo-scope checks over repo-level facts; the only ones a workspace root runs. */
export const ROOT_CHECKS: ReadonlyArray<readonly [string, RepoCheck]> = [
  ["banned-deps", bannedDeps],
  ["shadow-configs", shadowConfigs],
  ["required-files", requiredFiles],
  ["secrets", secrets],
];

export const ALL_CHECK_IDS: readonly string[] = [
  ...FILE_CHECKS.map(([id]) => id),
  "patterns",
  ...TREE_CHECKS.map(([id]) => id),
  ...ROOT_CHECKS.map(([id]) => id),
];

/** Honour --only, and skip whatever the linter owns. */
export function selected(id: string, ownedByLinter: readonly string[], only: string): boolean {
  if (ownedByLinter.includes(id)) return false;
  return only === "" || only.split(",").includes(id);
}
