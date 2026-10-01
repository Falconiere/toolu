/** Native gate-status behavior against real project state. */
import { expect, test } from "bun:test";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { hookEnv } from "../../dispatch/__tests__/dispatch-harness.ts";
import { gateStatusModule } from "../gate-status.ts";
import { bashPayload, runNative, type Call, type Side } from "./gates-harness.ts";

const GATE = ".claude/tmp/quality-gate-status.json";

function seed(sb: Sandbox, doc: object, rel = GATE): void {
  const path = sb.path(rel);
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, `${JSON.stringify(doc, null, 2)}\n`);
}

const legacy = (source: string, status: string) => (sb: Sandbox) =>
  seed(sb, { status, source, reason: "seeded by test", updatedAt: "2026-01-01T00:00:00Z" });

const tsEntry = (sb: Sandbox) =>
  seed(sb, {
    status: "failing",
    reason: "bad ts",
    source: "ts-quality-hook",
    file: "/p/a.ts",
    violations: "viol\n",
    entries: {
      "/p/a.ts": {
        source: "ts-quality-hook",
        reason: "bad ts",
        violations: "viol\n",
        updatedAt: "2026-01-01T00:00:00Z",
      },
    },
    updatedAt: "2026-01-01T00:00:00Z",
  });

const exit = (command: string, code: number) =>
  bashPayload(command, { metadata: { exit_code: code } });

type Scenario = {
  name: string;
  payloads: string[];
  setup?: (sb: Sandbox) => void;
  env?: Record<string, string>;
  status?: "passing" | "failing";
};

const CASES: Scenario[] = [
  {
    name: "non-Bash tool is a no-op",
    payloads: [JSON.stringify({ tool_name: "Write", tool_input: { file_path: "a.ts" } })],
  },
  { name: "non-quality command creates no gate file", payloads: [exit("ls -la", 0)] },
  {
    name: "first passing quality command writes a passing gate",
    payloads: [exit("cargo clippy", 0)],
    status: "passing",
  },
  {
    name: "failing quality command records and advises",
    payloads: [exit("bun test", 1)],
    status: "failing",
  },
  {
    name: "Codex host writes the Codex state path",
    payloads: [
      JSON.stringify({
        session_id: "s",
        turn_id: "t",
        hook_event_name: "PostToolUse",
        tool_name: "Bash",
        tool_input: { command: "bun test" },
        tool_response: { metadata: { exit_code: 1 }, stdout: "", stderr: "failed" },
      }),
    ],
    env: { TOOLU_HOST_OVERRIDE: "codex" },
    status: "failing",
  },
  {
    name: "a failing rust-quality-hook gate survives a passing quality command",
    setup: legacy("rust-quality-hook", "failing"),
    payloads: [exit("cargo clippy", 0)],
    status: "failing",
  },
  {
    name: "a failing ts-quality-hook gate survives a passing quality command",
    setup: legacy("ts-quality-hook", "failing"),
    payloads: [exit("bun run check", 0)],
    status: "failing",
  },
  {
    name: "a failing command slot flips to passing on a passing command",
    payloads: [exit("bun test", 1), exit("cargo test", 0)],
    status: "passing",
  },
  {
    name: "a failure is recorded beside a file hook's entry",
    setup: tsEntry,
    payloads: [exit("bun test", 1)],
    status: "failing",
  },
  {
    name: "a pass clears only the command slot",
    setup: tsEntry,
    payloads: [exit("cargo test", 0)],
    status: "failing",
  },
  {
    name: "a passing quality-hook gate is overwritten by a failing command",
    setup: legacy("ts-quality-hook", "passing"),
    payloads: [exit("bun test", 1)],
    status: "failing",
  },
  {
    name: "an existing passing gate is left as it is by another pass",
    payloads: [exit("tsc", 0), exit("bun test", 0)],
    status: "passing",
  },
  ...[
    "cargo test",
    "  cargo clippy",
    "cd crate && cargo test",
    "bun run check",
    "cd web && bun test",
    "find . | cargo test",
    "foo | tsc",
    "tsc --noEmit",
  ].map((command) => ({
    name: `TRIGGER ${command}`,
    payloads: [exit(command, 0)],
    status: "passing" as const,
  })),
  ...["cat tsconfig.json", "ls tooling/foo/test.sh", "vitests-helper", "cattsc"].map((command) => ({
    name: `NO-TRIGGER ${command}`,
    payloads: [exit(command, 0)],
  })),
  { name: "no reported exit status records nothing", payloads: [bashPayload("bun test", {})] },
  {
    name: "a fractional exit status records nothing",
    payloads: [bashPayload("bun test", { exit_code: 1.5 })],
  },
  {
    name: "Cursor's tool_output exit status is read",
    status: "failing",
    payloads: [
      JSON.stringify({
        tool_name: "Shell",
        tool_input: { command: "bun test" },
        tool_output: '{"exitCode":1}',
      }),
    ],
  },
];

async function run(s: Scenario): Promise<Side> {
  using sb = createSandbox({ git: true });
  s.setup?.(sb);
  const env = { ...hookEnv(sb), ...s.env };
  const calls: Call[] = s.payloads.map((stdin) => ({ stdin, env }));
  return await runNative(sb, gateStatusModule, calls);
}

for (const s of CASES) {
  test.concurrent(s.name, async () => {
    const result = await run(s);
    expect(result.exitCode).toBe(0);
    const gate =
      result.state[
        s.env?.TOOLU_HOST_OVERRIDE === "codex" ? ".codex/tmp/quality-gate-status.json" : GATE
      ];
    if (s.status === undefined) expect(gate).toBeUndefined();
    else expect(JSON.parse(gate ?? "null")).toMatchObject({ status: s.status });
  });
}

test.concurrent("the failure advisory names the command and its exit status", async () => {
  const ts = await run({ name: "", payloads: [exit("bun test", 1)] });
  expect(JSON.parse(ts.stdout)).toEqual({
    hookSpecificOutput: {
      hookEventName: "PostToolUse",
      additionalContext:
        "Global quality gate failing. Fix all errors/warnings/tests before new tasks.\\nFailed: bun test (exit 1)",
    },
  });
  expect(JSON.parse(ts.state[GATE] ?? "{}")).toMatchObject({
    status: "failing",
    source: "gate-status-hook",
  });
});

/** #283 items 6 and 7: after a failing `bun test`, these lines must not clear the gate. */
const MUST_STAY_FAILING: Record<string, string> = {
  "283-6a prose naming a quality command": 'echo "remember to run bun test later"',
  "283-7a quality command piped into tail": "bun test 2>&1 | tail -20",
  "an unproven command beside a proven one": "bun test 2>&1 | tail; bun run lint",
  "a quality command behind ||": "bun test || true",
  "a quality command followed by ;": "bun test; echo done",
};

for (const [name, command] of Object.entries(MUST_STAY_FAILING)) {
  test.concurrent(`#283: ${name} leaves a failing gate failing`, async () => {
    const ts = await run({ name, payloads: [exit("bun test", 1), exit(command, 0)] });
    expect(JSON.parse(ts.state[GATE] ?? "{}")).toMatchObject({ status: "failing" });
  });
}

test.concurrent("a failing tail after a quality command records nothing", async () => {
  const ts = await run({ name: "", payloads: [exit("bun test 2>&1 | tail -5", 1)] });
  expect(ts.state[GATE]).toBeUndefined();
  expect(ts.stdout).toBe("");
});

test.concurrent("a failing line of proven quality commands records the failure", async () => {
  const ts = await run({ name: "", payloads: [exit("bun run lint && bun test", 1)] });
  expect(JSON.parse(ts.state[GATE] ?? "{}")).toMatchObject({ status: "failing" });
});
