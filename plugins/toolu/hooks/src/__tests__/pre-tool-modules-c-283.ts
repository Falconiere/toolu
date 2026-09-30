/**
 * #283 items 8 and 9 on push-review, plan-ledger and docs-sync (#262). Item 8:
 * pushes bash's text match missed, which the parsed command finds. Item 9: a
 * `git -C "<path with space>" push`, or a `-C` chain whose first `-C` is not
 * the push's, is judged on the repository it pushes, not on the cwd's. Each
 * golden is bash's known-wrong answer, and the bundle must differ from it.
 */
import { join } from "node:path";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import {
  BASE,
  commitFile,
  diffShaIn,
  featureRepo,
  gitIn,
  group,
  writeIn,
  type GateCase,
} from "./pre-tool-modules-c-cases.ts";
import { step } from "./pre-tool-modules-c-plan-ledger-kit.ts";
import { STRICT, writeState } from "./pre-tool-modules-c-push-review-lib.ts";

const ITEM_8: readonly (readonly [string, string])[] = [
  ["timeout", "timeout 120 git push"],
  ["nice", "nice -n 10 git push"],
  ["sudo -u", "sudo -u me git push"],
  ["env -i", "env -i PATH=/usr/bin git push"],
  ["an absolute git path", "/usr/bin/git push"],
  ["--git-dir with a separate value", "git --git-dir .git push"],
  ["bash -c", 'bash -c "git push"'],
  ["eval", "eval git push"],
  ["xargs", "echo main | xargs git push origin"],
  ["a backslash-newline continuation", "git \\\n  push"],
];

const MISSED = "#283 item 8: bash's text match missed this push";
const WRONG_REPO = "#283 item 9: bash judged the cwd's repository, not the pushed one";

/** Only `gate` speaks: the other two push gates are off. */
function only(gate: "pushReview" | "planLedger" | "docsSync", preset = "strict"): object {
  const off = { mode: "off" };
  const gates = { pushReview: off, planLedger: off, docsSync: off };
  return { version: 1, gates: { ...gates, preset, [gate]: {} } };
}

/** A red plan-ledger step for the checked-out branch of `dir`. */
function redLedger(dir: string, slug: string): void {
  const steps = [step("s1", "first", "red", diffShaIn(dir))];
  const ledger = { version: 1, branch: slug, base_branch: BASE, summary: { total: 1 }, steps };
  writeIn(dir, `.claude/tmp/plan-ledger/${slug}.json`, JSON.stringify(ledger));
}

const pr = group({ config: STRICT, setup: featureRepo, deviation: MISSED });
const pl = group({
  config: only("planLedger"),
  deviation: MISSED,
  setup: (sb) => {
    featureRepo(sb);
    redLedger(sb.project, "feat_example");
  },
});
const ds = group({
  config: only("docsSync", "balanced"),
  deviation: MISSED,
  setup: (sb) => {
    featureRepo(sb);
    commitFile(sb, "src/tool.ts", "export {};");
  },
});

const ITEM_8_CASES: GateCase[] = ITEM_8.flatMap(([label, command]) => [
  pr({
    name: `push-review: #283 ${label}: a push is reviewed`,
    command,
    expect: "deny",
    has: ["Code review required before push"],
  }),
  pl({
    name: `plan-ledger: #283 ${label}: a push waits for the plan`,
    command,
    expect: "deny",
    has: ["steps not fresh-green", "s1: red"],
  }),
  ds({
    name: `docs-sync: #283 ${label}: a push is nudged`,
    command,
    expect: "advisory",
    has: ["docs-sync: this branch changes code"],
  }),
]);

/** `<root>/my wt/wt`: a worktree of the project on `feat/wt`, one commit of `path` ahead of base. */
function worktree(sb: Sandbox, path = "wt.txt"): string {
  const wt = join(sb.root, "my wt", "wt");
  gitIn(sb.project, ["worktree", "add", "-q", "-b", "feat/wt", wt, BASE]);
  commitFile(sb, path, "wt", wt);
  return wt;
}

const wtOf = (sb: Sandbox): string => join(sb.root, "my wt", "wt");
const quotedPush = (sb: Sandbox): string => `git -C "${wtOf(sb)}" push`;
const chainPush = (sb: Sandbox): string =>
  `git -C "${sb.project}" log --grep push; git -C "${wtOf(sb)}" push`;

const bashPush = (command: (sb: Sandbox) => string) => (sb: Sandbox) => ({
  kind: "tool" as const,
  event: "PreToolUse" as const,
  toolName: "Bash",
  toolInput: { command: command(sb) },
});

const item9 = group({ deviation: WRONG_REPO });

const ITEM_9_CASES: GateCase[] = [
  item9({
    name: "push-review: #283 -C with a space: the worktree's clean review allows",
    config: STRICT,
    setup: (sb) => {
      featureRepo(sb);
      writeState(worktree(sb));
    },
    fixture: bashPush(quotedPush),
    expect: "silent",
  }),
  item9({
    name: "push-review: #283 -C with a space: a review only in the cwd repo does not count",
    config: STRICT,
    setup: (sb) => {
      featureRepo(sb);
      writeState(sb.project);
      worktree(sb);
    },
    fixture: bashPush(quotedPush),
    expect: "deny",
    has: ["Code review required before push", "my wt/wt/.claude/tmp/push-review/feat_wt.json"],
  }),
  item9({
    name: "push-review: #283 a -C chain is judged on the pushed repository",
    config: STRICT,
    setup: (sb) => {
      featureRepo(sb);
      writeState(worktree(sb));
    },
    fixture: bashPush(chainPush),
    expect: "silent",
  }),
  item9({
    name: "plan-ledger: #283 -C with a space: the worktree's red plan waits",
    config: only("planLedger"),
    setup: (sb) => {
      featureRepo(sb);
      redLedger(worktree(sb), "feat_wt");
    },
    fixture: bashPush(quotedPush),
    expect: "deny",
    has: ["steps not fresh-green", "s1: red"],
  }),
  item9({
    name: "docs-sync: #283 -C with a space: the worktree's code change is nudged",
    config: only("docsSync", "balanced"),
    setup: (sb) => {
      featureRepo(sb);
      commitFile(sb, "README.md", "docs");
      worktree(sb, "src/wt.ts");
    },
    fixture: bashPush(quotedPush),
    expect: "advisory",
    has: ["docs-sync: this branch changes code", "my wt/wt/.claude/tmp/docs-sync/feat_wt.json"],
  }),
];

const CODEX_CASES: GateCase[] = [
  pr({
    name: "push-review: #283 timeout on Codex: a push is reviewed",
    host: "codex",
    command: "timeout 120 git push",
    expect: "deny",
    has: ["Code review required before push", ".codex/tmp/push-review/feat_example.json"],
  }),
];

export const CASES_283: GateCase[] = [...ITEM_8_CASES, ...ITEM_9_CASES, ...CODEX_CASES];
