/**
 * AC-5 (#259): quality commands are recognised from the parsed command line.
 * Every TRIGGER and NO-TRIGGER case of `gate-status.bats` keeps its answer
 * (checked against the shipped regex, run by the real `grep -E`), the runner
 * and shell forms the regex caught by accident stay recognised, and prose or a
 * function body no longer counts (#283 item 6).
 */
import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { analyzeShell } from "../../shell/shell-parse.ts";
import { qualityCommands } from "../quality-command.ts";

const GATE_STATUS = readFileSync(
  resolve(import.meta.dir, "../../../../../plugins/toolu/hooks/post-tools/modules/gate-status.sh"),
  "utf8",
);
const REGEX_LINES = GATE_STATUS.split("\n").filter((line) => line.startsWith("GATE_TRIGGER_"));

/** The shipped text match: `echo "$command" | grep -qE "${PREFIX}(${ALTERNATION})${SUFFIX}"`. */
function bashMatches(command: string): boolean {
  const script = `${REGEX_LINES.join("\n")}\necho "$1" | grep -qE "\${GATE_TRIGGER_PREFIX}(\${GATE_TRIGGER_ALTERNATION})\${GATE_TRIGGER_SUFFIX}"`;
  return spawnSync("bash", ["-c", script, "gate", command]).status === 0;
}

function labels(command: string): string[] {
  return qualityCommands(analyzeShell(command)).map((q) => q.label);
}

test("the shipped regex is read from gate-status.sh", () => {
  expect(REGEX_LINES).toHaveLength(3);
});

/** From gate-status.bats TRIGGER cases, then forms the regex matched through a runner or shell. */
const TRIGGERS: Record<string, string> = {
  "cargo test": "cargo test",
  "  cargo clippy": "cargo clippy",
  "cd crate && cargo test": "cargo test",
  "bun test": "bun test",
  "bun run check": "bun run check",
  "cd web && bun test": "bun test",
  "find . | cargo test": "cargo test",
  "foo | tsc": "tsc",
  tsc: "tsc",
  "tsc --noEmit": "tsc",
  "bun run ts:check:fix": "bun run ts:check:fix",
  "cargo nextest run": "cargo nextest",
  "vitest run": "vitest",
  "jest --ci": "jest",
  "tools/api/check.sh": "tools/api/check.sh",
  "bash tools/api/check.sh": "tools/api/check.sh",
  "sh tools/web/test.sh --fast": "tools/web/test.sh",
  "bash ./scripts/ts-check.sh": "./scripts/ts-check.sh",
  "./scripts/ts-check.sh": "./scripts/ts-check.sh",
  "npx tsc --noEmit": "tsc",
  "npx -y vitest run": "vitest",
  "bunx vitest run": "vitest",
  "bun x tsc": "tsc",
  "pnpm exec jest": "jest",
  "yarn tsc -b": "tsc",
};

for (const [command, label] of Object.entries(TRIGGERS)) {
  test.concurrent(`TRIGGER ${command}`, () => {
    expect(bashMatches(command)).toBe(true);
    expect(labels(command)).toEqual([label]);
  });
}

/** Forms the regex missed that the parsed command reaches (wrappers, paths, nested shells). */
const PARSED_ONLY: Record<string, string> = {
  "./tools/api/check.sh": "tools/api/check.sh",
  "timeout 600 bun test": "bun test",
  "cargo +nightly clippy": "cargo clippy",
  "/usr/local/bin/tsc -p .": "tsc",
};

for (const [command, label] of Object.entries(PARSED_ONLY)) {
  test.concurrent(`TRIGGER (parsed only) ${command}`, () => {
    expect(labels(command)).toEqual([label]);
  });
}

test.concurrent("bash -c and a sudo wrapper are followed", () => {
  expect(labels("bash -c 'bun test'")).toEqual(["bun test"]);
  expect(labels("sudo -u ci cargo test")).toEqual(["cargo test"]);
});

test.concurrent("every quality command in the line is reported, in order", () => {
  expect(labels("bun run lint && bun test 2>&1 | tail -5")).toEqual(["bun run lint", "bun test"]);
});

/** gate-status.bats NO-TRIGGER cases: neither side matches. */
const NO_TRIGGERS = ["cat tsconfig.json", "ls tooling/foo/test.sh", "vitests-helper", "cattsc"];

for (const command of NO_TRIGGERS) {
  test.concurrent(`NO-TRIGGER ${command}`, () => {
    expect(bashMatches(command)).toBe(false);
    expect(labels(command)).toEqual([]);
  });
}

/** Text that names a quality command without running one: the regex matched these (#283 item 6). */
const PROSE = [
  'echo "remember to run bun test later"',
  'git commit -m "fix: make cargo test pass"',
  "grep -rn tsc docs",
];

for (const command of PROSE) {
  test.concurrent(`prose is not a run: ${command}`, () => {
    expect(bashMatches(command)).toBe(true);
    expect(labels(command)).toEqual([]);
  });
}

test.concurrent("other commands of the same tools are not quality commands", () => {
  for (const command of ["bun run dev", "bun install", "cargo run", "npx prettier --check ."]) {
    expect(labels(command)).toEqual([]);
  }
});

test.concurrent("a function body is defined, not run", () => {
  expect(labels("t() { bun test; }")).toEqual([]);
});

test.concurrent("a dynamic command name is never a quality command", () => {
  expect(labels("$RUNNER test")).toEqual([]);
});

/**
 * Where argv reading and the text regex part ways on a genuine invocation,
 * pinned both ways (documented in quality-command.ts and gates.md).
 */
const DIVERGENT: Record<string, { bash: boolean; ts: string[] }> = {
  // Broader: a path to the tool, a toolchain selector, a wrapper script by path.
  "./node_modules/.bin/tsc --noEmit": { bash: false, ts: ["tsc"] },
  "/usr/local/bin/cargo test": { bash: false, ts: ["cargo test"] },
  "cargo +nightly test": { bash: false, ts: ["cargo test"] },
  "/repo/tools/api/check.sh": { bash: false, ts: ["tools/api/check.sh"] },
  // Narrower: a runner subcommand, a runner option with a value, a shell option.
  "yarn run vitest": { bash: true, ts: [] },
  "npx -p typescript tsc": { bash: true, ts: [] },
  "bash -x ./scripts/ts-check.sh": { bash: true, ts: [] },
};

for (const [command, want] of Object.entries(DIVERGENT)) {
  test.concurrent(`divergent form: ${command}`, () => {
    expect(bashMatches(command)).toBe(want.bash);
    expect(labels(command)).toEqual(want.ts);
  });
}
