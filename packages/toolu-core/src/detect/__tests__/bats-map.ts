/**
 * Where each `detect.bats` / `detect-plugin-active.bats` case lives in bun
 * test (#254 AC-9). A bats name starts with the function it exercises (or
 * `active:`); each prefix maps to the repository-relative file that holds its
 * cases. `fixtureFn` marks cases carried by the #284 harvest in
 * `tooling/fixtures/shell/bats-parity.json`, replayed by the shell parity suites
 * through the production detect functions.
 */
export interface BatsHome {
  /** Repository-relative test or fixture file holding the ported cases. */
  readonly home: string;
  /** The `fn` the harvested fixture records, when `home` is that fixture. */
  readonly fixtureFn?: string;
}

const PROJECT = "packages/toolu-core/src/detect/__tests__/detect-project.test.ts";
const LINES = "packages/toolu-core/src/detect/__tests__/detect-lines.test.ts";
const BRANCH = "packages/toolu-core/src/detect/__tests__/detect-branch.test.ts";
const FIXTURE = "tooling/fixtures/shell/bats-parity.json";

export const BATS_FILES = [
  "plugins/toolu/hooks/lib/__tests__/detect.bats",
  "plugins/toolu/hooks/lib/__tests__/detect-plugin-active.bats",
] as const;

export const BATS_MAP: Readonly<Record<string, BatsHome>> = {
  detect_project_root: { home: PROJECT },
  detect_project_name: { home: PROJECT },
  detect_node_pm: { home: PROJECT },
  detect_rust: { home: PROJECT },
  detect_python: { home: PROJECT },
  detect_ts: { home: PROJECT },
  detect_ts_linter: { home: PROJECT },
  detect_python_linter: { home: PROJECT },
  detect_clippy: { home: PROJECT },
  to_relative_path: { home: PROJECT },
  count_code_lines: { home: LINES },
  count_python_code_lines: { home: LINES },
  branch_slug: { home: BRANCH },
  detect_base_branch: { home: BRANCH },
  strip_heredocs: { home: "packages/toolu-core/src/detect/__tests__/detect-git.test.ts" },
  read_list: { home: "packages/toolu-core/src/config/__tests__/settings.test.ts" },
  detect_plugin_installed: {
    home: "packages/toolu-core/src/registry/__tests__/registry-gate-parity.test.ts",
  },
  active: { home: "packages/toolu-core/src/registry/__tests__/registry-gate-parity.test.ts" },
  is_git_push: { home: FIXTURE, fixtureFn: "is_git_push" },
  is_git_commit: { home: FIXTURE, fixtureFn: "is_git_commit" },
  push_target_root: { home: FIXTURE, fixtureFn: "push_target_root" },
  push_target_branch: { home: FIXTURE, fixtureFn: "push_target_branch" },
  bash_write_targets: { home: FIXTURE, fixtureFn: "bash_write_targets" },
};

/** The map key a bats test name belongs to: its leading identifier. */
export function batsPrefix(name: string): string {
  return /^[a-z_]+/.exec(name)?.[0] ?? "";
}
