/**
 * AC-5 (#259): quality commands are recognised from the parsed command line.
 * Quality invocations through runners and shell forms are recognized, while
 * prose and function definitions do not count (#283 item 6).
 */
import { expect, test } from "bun:test";
import { analyzeShell } from "../../shell/shell-parse.ts";
import { qualityCommands } from "../quality-command.ts";

function labels(command: string): string[] {
  return qualityCommands(analyzeShell(command)).map((q) => q.label);
}

/** Trigger cases, including commands reached through runners or shells. */
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
    expect(labels(command)).toEqual([label]);
  });
}

/** Parsed commands reached through wrappers, paths and nested shells. */
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

/** Commands that must not trigger the gate. */
const NO_TRIGGERS = ["cat tsconfig.json", "ls tooling/foo/test.sh", "vitests-helper", "cattsc"];

for (const command of NO_TRIGGERS) {
  test.concurrent(`NO-TRIGGER ${command}`, () => {
    expect(labels(command)).toEqual([]);
  });
}

/** Text that names a quality command without running one (#283 item 6). */
const PROSE = [
  'echo "remember to run bun test later"',
  'git commit -m "fix: make cargo test pass"',
  "grep -rn tsc docs",
];

for (const command of PROSE) {
  test.concurrent(`prose is not a run: ${command}`, () => {
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

/** Path, toolchain and option boundaries in native argv reading. */
const BOUNDARIES: Record<string, string[]> = {
  // Broader: a path to the tool, a toolchain selector, a wrapper script by path.
  "./node_modules/.bin/tsc --noEmit": ["tsc"],
  "/usr/local/bin/cargo test": ["cargo test"],
  "cargo +nightly test": ["cargo test"],
  "/repo/tools/api/check.sh": ["tools/api/check.sh"],
  // Narrower: a runner subcommand, a runner option with a value, a shell option.
  "yarn run vitest": [],
  "npx -p typescript tsc": [],
  "bash -x ./scripts/ts-check.sh": [],
};

for (const [command, want] of Object.entries(BOUNDARIES)) {
  test.concurrent(`argv boundary: ${command}`, () => {
    expect(labels(command)).toEqual(want);
  });
}
