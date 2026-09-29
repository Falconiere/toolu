/**
 * gate-status (#259): the native module against the shipped `gate-status.sh`,
 * both under their dispatcher, from the same sandbox state. Every
 * `gate-status.bats` scenario gives the same stdout and the same gate state
 * (AC-1). The #283 item 6 and 7 fixtures give the correct state where the bash
 * baseline is recorded wrong (AC-3).
 */
import { expect, test } from "bun:test";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { hookEnv } from "../../dispatch/__tests__/dispatch-harness.ts";
import { gateStatusModule } from "../gate-status.ts";
import { bashPayload, bothSides, type Call } from "./gates-harness.ts";

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
  /** State files only the TypeScript side writes, by a documented #255 deviation. */
  tsOnly?: string[];
};

const PARITY: Scenario[] = [
  {
    name: "non-Bash tool is a no-op",
    payloads: [JSON.stringify({ tool_name: "Write", tool_input: { file_path: "a.ts" } })],
  },
  { name: "non-quality command creates no gate file", payloads: [exit("ls -la", 0)] },
  {
    name: "first passing quality command writes a passing gate",
    payloads: [exit("cargo clippy", 0)],
  },
  { name: "failing quality command records and advises", payloads: [exit("bun test", 1)] },
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
  },
  {
    name: "a failing rust-quality-hook gate survives a passing quality command",
    setup: legacy("rust-quality-hook", "failing"),
    payloads: [exit("cargo clippy", 0)],
  },
  {
    name: "a failing ts-quality-hook gate survives a passing quality command",
    setup: legacy("ts-quality-hook", "failing"),
    payloads: [exit("bun run check", 0)],
  },
  {
    name: "a failing command slot flips to passing on a passing command",
    payloads: [exit("bun test", 1), exit("cargo test", 0)],
  },
  {
    name: "a failure is recorded beside a file hook's entry",
    setup: tsEntry,
    payloads: [exit("bun test", 1)],
  },
  {
    name: "a pass clears only the command slot",
    setup: tsEntry,
    payloads: [exit("cargo test", 0)],
  },
  {
    name: "a passing quality-hook gate is overwritten by a failing command",
    setup: legacy("ts-quality-hook", "passing"),
    payloads: [exit("bun test", 1)],
    // A passing record with a `reason` fails the strict v1 schema: `recordGateFailure`
    // replaces it and leaves a breadcrumb (gate-file.ts, "Where it goes past bash").
    tsOnly: [`${GATE}.dropped.log`],
  },
  {
    name: "an existing passing gate is left as it is by another pass",
    payloads: [exit("tsc", 0), exit("bun test", 0)],
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
  ].map((command) => ({ name: `TRIGGER ${command}`, payloads: [exit(command, 0)] })),
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
    payloads: [
      JSON.stringify({
        tool_name: "Shell",
        tool_input: { command: "bun test" },
        tool_output: '{"exitCode":1}',
      }),
    ],
  },
];

async function run(s: Scenario): Promise<Awaited<ReturnType<typeof bothSides>>> {
  using sb = createSandbox({ git: true });
  s.setup?.(sb);
  const env = { ...hookEnv(sb), ...s.env };
  const calls: Call[] = s.payloads.map((stdin) => ({ stdin, env }));
  return bothSides(sb, "gate-status", gateStatusModule, calls);
}

for (const s of PARITY) {
  test.concurrent(s.name, async () => {
    const { bash, ts } = await run(s);
    const state = { ...ts.state };
    for (const file of s.tsOnly ?? []) {
      expect(state[file]).toBeDefined();
      delete state[file];
    }
    expect({ ...ts, state }).toEqual(bash);
  });
}

test.concurrent("the failure advisory names the command and its exit status", async () => {
  const { ts } = await run({ name: "", payloads: [exit("bun test", 1)] });
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
    const { bash, ts } = await run({ name, payloads: [exit("bun test", 1), exit(command, 0)] });
    expect(JSON.parse(ts.state[GATE] ?? "{}")).toMatchObject({ status: "failing" });
    // The bash baseline is the known-wrong one: it cleared the gate.
    expect(JSON.parse(bash.state[GATE] ?? "{}")).toMatchObject({ status: "passing" });
  });
}

test.concurrent("a failing tail after a quality command records nothing", async () => {
  const { bash, ts } = await run({ name: "", payloads: [exit("bun test 2>&1 | tail -5", 1)] });
  expect(ts.state[GATE]).toBeUndefined();
  expect(ts.stdout).toBe("");
  expect(JSON.parse(bash.state[GATE] ?? "{}")).toMatchObject({ status: "failing" });
});

test.concurrent("a failing line of proven quality commands records the failure", async () => {
  const { bash, ts } = await run({ name: "", payloads: [exit("bun run lint && bun test", 1)] });
  expect(ts).toEqual(bash);
  expect(JSON.parse(ts.state[GATE] ?? "{}")).toMatchObject({ status: "failing" });
});
