/**
 * Twin-repo scenario runner for the ledger CLI parity suites (#256). Each
 * scenario runs twice, once through `plan-ledger.sh` and once through the
 * TypeScript entry. Each run gets a fresh sandbox holding the same repo as the
 * bats suites: `main` has base.txt, and `feat/x` adds feature.txt. After every
 * action it snapshots the exit code, stdout, the lines the CLI authors on
 * stderr, and the state files. The two snapshot lists must be equal once
 * sandbox paths, timestamps and durations are normalized.
 */
import { existsSync, readFileSync } from "node:fs";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { childEnv, run, type EnvPatch } from "@toolu/conformance/harness/spawn";
import type { CommandResult, LedgerOptions } from "../ledger-io.ts";
import { LIB } from "./ledger-parity-helpers.ts";

export const LEDGER = ".claude/tmp/plan-ledger/feat_x.json";
export const TELEMETRY = ".claude/tmp/telemetry/feat_x.jsonl";

export type Action = {
  argv: (sb: Sandbox) => string[];
  before?: (sb: Sandbox) => void;
  /** Extra env, or a function of the sandbox for values that name its paths. */
  env?: EnvPatch | ((sb: Sandbox) => EnvPatch);
  /** Relative to the project (or a function of the sandbox); default the project root. */
  cwd?: string | ((sb: Sandbox) => string);
  /** Which bash CLI: default plan-ledger.sh. */
  cli?: string;
};

export type Scenario = {
  setup?: (sb: Sandbox) => void;
  actions: Action[];
  /** Extra project-relative files to snapshot after each action. */
  files?: string[];
  /** Make the project a non-repo (no git init). */
  noRepo?: boolean;
};

export type TsCli = (
  argv: string[],
  options: LedgerOptions,
) => Promise<CommandResult> | CommandResult;

export type Snapshot = {
  code: number;
  stdout: string;
  stderr: string[];
  files: Record<string, string | null>;
};

const STAMP = /\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ/g;
const AUTHORED = /^(plan-ledger|plan-ledger-parse|preflight|verdict|telemetry)(:| --self-test:)/;

function normalize(text: string, sb: Sandbox): string {
  return text
    .replaceAll(sb.root, "<ROOT>")
    .replace(STAMP, "<T>")
    .replace(/\((\d+)s\)/g, "(Ns)")
    .replace(/"duration_s":\d+/g, '"duration_s":N');
}

export function baseRepo(sb: Sandbox): void {
  sb.write("feature.txt", "feature\n");
  sb.git("checkout", "-q", "-b", "feat/x");
  sb.git("add", "feature.txt");
  sb.git("commit", "-qm", "feature");
}

/** Stage everything and commit: the ledger measures the committed diff only. */
export function commit(sb: Sandbox, message = "change"): void {
  sb.git("add", "-A", "--", ".", ":!.claude", ":!plan.md", ":!spec.md");
  sb.git("commit", "-qm", message);
}

function sandbox(scenario: Scenario): Sandbox {
  if (scenario.noRepo === true) return createSandbox();
  const sb = createSandbox({ git: true, files: { "base.txt": "base\n" } });
  baseRepo(sb);
  return sb;
}

async function play(scenario: Scenario, impl: "bash" | TsCli): Promise<Snapshot[]> {
  using sb = sandbox(scenario);
  scenario.setup?.(sb);
  const out: Snapshot[] = [];
  for (const action of scenario.actions) {
    action.before?.(sb);
    const cwd =
      action.cwd === undefined
        ? sb.project
        : typeof action.cwd === "string"
          ? sb.path(action.cwd)
          : action.cwd(sb);
    const env: EnvPatch = {
      PUSH_REVIEW_BASE: "main",
      HOME: sb.home,
      TOOLU_HOST_OVERRIDE: "claude",
      ...(typeof action.env === "function" ? action.env(sb) : action.env),
    };
    const argv = action.argv(sb);
    const res =
      impl === "bash"
        ? await run(["bash", `${LIB}/${action.cli ?? "plan-ledger.sh"}`, ...argv], {
            cwd,
            env,
            timeoutMs: 60_000,
          })
        : await impl(argv, { cwd, env: childEnv(env) });
    const files: Record<string, string | null> = {};
    for (const rel of [LEDGER, TELEMETRY, ...(scenario.files ?? [])]) {
      const abs = sb.path(rel);
      files[rel] = existsSync(abs) ? normalize(readFileSync(abs, "utf8"), sb) : null;
    }
    out.push({
      code: res.exitCode,
      stdout: normalize(res.stdout, sb),
      stderr: res.stderr
        .split("\n")
        .filter((line) => AUTHORED.test(line))
        .map((line) => normalize(line, sb)),
      files,
    });
  }
  return out;
}

/** Both implementations' snapshots, bash first. */
export async function twin(scenario: Scenario, ts: TsCli): Promise<[Snapshot[], Snapshot[]]> {
  const [bash, port] = await Promise.all([play(scenario, "bash"), play(scenario, ts)]);
  return [bash, port];
}

/** A plan doc with the given steps array. */
export function planDoc(steps: unknown[], header = ""): string {
  return `# Fixture Plan\n\n${header}\n\n## Steps (machine-readable)\n\n\`\`\`json\n${JSON.stringify(steps, null, 2)}\n\`\`\`\n`;
}

export const step = (id: string, check: string, extra: Record<string, unknown> = {}) => ({
  id,
  title: `${id} title`,
  check,
  ...extra,
});

export function readText(sb: Sandbox, rel: string): string {
  return readFileSync(sb.path(rel), "utf8");
}
