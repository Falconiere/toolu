/**
 * Fixture for the pinned-host pr-babysit scenarios (#357): an isolated project
 * on a PR branch with a failing test and a bare origin, the scripted controller
 * and fixer steps, and readers for what the helpers and the fixer left behind.
 * The fixer finds the provider in the profile's global config, as a user's
 * own would be.
 */
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { delimiter, dirname, join } from "node:path";
import { z } from "zod";
import { SELECTION, selection } from "./install-host.ts";
import type { ScriptStep, Scripts } from "./provider.ts";
import { entrySession, npmSpec, ROOT, type EntryContext } from "./scenarios-entry.ts";
import type { ProbeSession } from "./session.ts";

export const SKILL = "pr-babysit-babysit-73c340c6";
export const SLOT = "falconiere-toolu-165";
export const BRANCH = "feat/sum";
const SNAPSHOT = join(
  ROOT,
  "plugins/pr-babysit/scripts/__tests__/fixtures/snapshots/toolu-165.json",
);
const HELPER = '"$TOOLU_BUN" --no-env-file "$TOOLU_PLUGIN_ROOT_PR_BABYSIT/hooks/dist';
/** The controller waits for the fixer inside one bash call; the run gets more. */
const WAIT_SECONDS = 300;
export const RUN_MS = 900_000;
/** A command no other process on the machine runs, so a survivor is unambiguous. */
export const SLEEP = "sleep 117";

const TEST_FILE = `import { expect, test } from "bun:test";
import { sum } from "./sum.ts";

test("sum adds", () => {
  expect(sum(2, 3)).toBe(5);
});
`;

export type Paths = {
  project: string;
  state: string;
  worktree: string;
  brief: string;
  report: string;
  log: string;
  items: string;
  snapshot: string;
};

export function paths(project: string): Paths {
  const base = join(project, ".opencode/tmp/pr-babysit", SLOT);
  return {
    project,
    state: `${base}.json`,
    worktree: `${base}.worktree`,
    brief: `${base}.fixer-r1g1.md`,
    report: `${base}.fixer-r1g1.report.json`,
    log: `${base}.fixer-r1g1.log`,
    items: join(project, "items.json"),
    snapshot: join(project, "snapshot.json"),
  };
}

export function bash(command: string, timeout?: number): ScriptStep {
  return {
    tool: "bash",
    args: { command, description: "babysit", ...(timeout === undefined ? {} : { timeout }) },
  };
}

/** One helper call: stdout to `<name>.json`, exit status appended to `exits.txt`. */
export function helper(name: string, args: string, timeout?: number): ScriptStep {
  return bash(
    `${HELPER}/${args} > ${name}.json; printf '${name}=%s\\n' "$?" >> exits.txt`,
    timeout,
  );
}

/** Tick on the captured snapshot, route with `--host opencode`, start the fixer. */
export function startSteps(p: Paths): ScriptStep[] {
  return [
    helper(
      "tick",
      `babysit-tick.js" --repo Falconiere/toolu --pr 165 --state-file '${p.state}' --snapshot-in '${p.snapshot}' --now 2026-09-19T12:00:00Z`,
    ),
    helper(
      "route",
      `babysit-route-fix.js" --items '${p.items}' --host opencode --state-file '${p.state}'`,
    ),
    helper(
      "start",
      `babysit-dispatch-fix.js" start --state-file '${p.state}' --plan route.json --items '${p.items}' --repo-root '${p.project}' --branch ${BRANCH} --base main`,
    ),
  ];
}

export function waitStep(p: Paths, name: string): ScriptStep {
  return helper(
    name,
    `babysit-dispatch-fix.js" wait --state-file '${p.state}' --timeout-seconds ${WAIT_SECONDS}`,
    (WAIT_SECONDS + 60) * 1000,
  );
}

/** The fixer's work: read the brief, fix, test, try the forbidden writes, commit, report. */
export function fixerSteps(p: Paths): ScriptStep[] {
  const file = join(p.worktree, "sum.ts");
  return [
    { tool: "read", args: { filePath: p.brief } },
    { tool: "read", args: { filePath: file } },
    { tool: "edit", args: { filePath: file, oldString: "a - b", newString: "a + b" } },
    bash('"$TOOLU_BUN" test sum.test.ts'),
    bash(`git push origin HEAD:${BRANCH}`),
    bash(`git -C '${p.worktree}' push origin HEAD:${BRANCH}`),
    bash("gh pr comment 165 --body fixed"),
    {
      tool: "task",
      args: { description: "push", prompt: "push the fix", subagent_type: "general" },
    },
    bash('git add sum.ts && git commit -q -m "fix(sum): address PR review feedback"'),
    bash(
      `"$TOOLU_BUN" "$TOOLU_PLUGIN_ROOT_PR_BABYSIT/hooks/dist/babysit-fixer-report.js" '${p.report}' done --note 'sum adds'`,
    ),
  ];
}

export function git(cwd: string, ...args: string[]): string {
  const res = spawnSync("git", args, { cwd, encoding: "utf8" });
  if (res.status !== 0) throw new Error(`git ${args.join(" ")}: ${res.stderr}`);
  return res.stdout.trim();
}

/** A project selecting pr-babysit, loading the packed package, on a PR branch with a failing test. */
export function babysitSession(ctx: EntryContext, scripts: (p: Paths) => Scripts): ProbeSession {
  const s = entrySession(ctx, {
    files: {
      [SELECTION]: selection(["pr-babysit"]),
      ".opencode/toolu.config.json": JSON.stringify({
        version: 1,
        prBabysit: {
          jev: false,
          routing: { opencode: Array.from({ length: 4 }, () => ({ model: "probe/scripted" })) },
        },
      }),
      "sum.ts": "export const sum = (a: number, b: number): number => a - b;\n",
    },
    scripts: (project) => scripts(paths(project)),
    config: () => ({ plugin: [npmSpec(ctx.tarball)], permission: { bash: "allow" } }),
  });
  // The dispatcher starts `opencode` from PATH: the pinned binary, not one the machine has.
  s.env.PATH = `${dirname(ctx.bin)}${delimiter}${process.env.PATH ?? ""}`;
  const p = paths(s.sb.project);
  const bare = s.outside("origin.git");
  git(s.sb.root, "init", "--quiet", "--bare", "-b", "main", bare);
  s.sb.git("remote", "add", "origin", bare);
  s.sb.git("push", "--quiet", "origin", "main");
  s.sb.git("checkout", "--quiet", "-b", BRANCH);
  s.sb.write("sum.test.ts", TEST_FILE);
  s.sb.git("add", "sum.test.ts");
  s.sb.git("commit", "--quiet", "-m", "test: sum adds");
  s.sb.git("push", "--quiet", "origin", BRANCH);
  const global = z.record(z.string(), z.unknown()).parse(JSON.parse(s.sb.read("opencode.json")));
  delete global["plugin"];
  const dir = join(s.env.XDG_CONFIG_HOME ?? join(s.sb.home, ".config"), "opencode");
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, "opencode.json"), JSON.stringify(global, null, 2));
  copyFileSync(SNAPSHOT, p.snapshot);
  writeFileSync(
    p.items,
    JSON.stringify({
      round: 1,
      items: [
        {
          id: "ci:test",
          kind: "ci",
          path: "sum.ts",
          severity: "high",
          task: "sum.test.ts fails because sum subtracts. Make sum add its arguments and run its test.",
        },
      ],
    }),
  );
  return s;
}

/** The bare origin `babysitSession` pushed to, and the PR branch head it holds. */
export function originOf(s: ProbeSession): { path: string; prHead: string } {
  const path = s.outside("origin.git");
  return { path, prHead: git(path, "rev-parse", BRANCH) };
}

const JsonRecord = z.record(z.string(), z.unknown());

export function json(path: string): Record<string, unknown> {
  return existsSync(path) ? JsonRecord.parse(JSON.parse(readFileSync(path, "utf8"))) : {};
}

export function text(path: string): string {
  return existsSync(path) ? readFileSync(path, "utf8") : "";
}

const Group = z.looseObject({
  host: z.string(),
  status: z.string(),
  reason: z.string().nullable(),
  error: z.string().optional(),
  pid: z.number().optional(),
});

/** The first fixer group of a dispatch status or state document. */
export function firstGroup(record: Record<string, unknown>): z.infer<typeof Group> | undefined {
  const source = record["fixer"] ?? record;
  const parsed = z.looseObject({ groups: z.array(Group) }).safeParse(source);
  return parsed.success ? parsed.data.groups[0] : undefined;
}

const ToolPart = z.looseObject({
  type: z.literal("tool_use"),
  part: z.looseObject({
    tool: z.string(),
    state: z.looseObject({ status: z.string(), input: z.unknown(), error: z.unknown() }),
  }),
});

function parseLine(line: string): unknown {
  try {
    return JSON.parse(line);
  } catch {
    // Non-JSON lines (host logs) are not tool events.
    return null;
  }
}

/** Tool calls of the fixer's own `opencode run --format json` log, with their inputs. */
export function fixerTools(
  log: string,
): Array<{ tool: string; status: string; input: string; error: string }> {
  return log.split("\n").flatMap((line) => {
    const parsed = ToolPart.safeParse(parseLine(line));
    if (!parsed.success) return [];
    const { tool, state } = parsed.data.part;
    return [
      {
        tool,
        status: state.status,
        input: JSON.stringify(state.input),
        error: typeof state.error === "string" ? state.error : "",
      },
    ];
  });
}

/** A call the host refused by a permission rule before running it. */
export function refused(
  calls: ReturnType<typeof fixerTools>,
  tool: string,
  needle: string,
): boolean {
  return calls.some(
    (c) =>
      c.tool === tool &&
      c.input.includes(needle) &&
      c.status === "error" &&
      c.error.includes("a rule which prevents you from using this specific tool call"),
  );
}

/** Live processes whose whole command line is `command`. */
export function running(command: string): number {
  const res = spawnSync("ps", ["-A", "-o", "args="], { encoding: "utf8" });
  return res.stdout.split("\n").filter((line) => line.trim() === command).length;
}

/** A fixer group the scenario left running never outlives it. */
export function reap(p: Paths): void {
  const pid = firstGroup(json(p.state))?.pid;
  if (pid === undefined) return;
  try {
    process.kill(-pid, "SIGKILL");
  } catch (error) {
    // ESRCH: the group already exited.
    if (!(error instanceof Error) || !("code" in error) || error.code !== "ESRCH") throw error;
  }
}
