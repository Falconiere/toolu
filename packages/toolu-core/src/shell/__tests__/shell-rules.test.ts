/**
 * Argv rules per simple command (#284, #283 item 4): a rule names a command and
 * the tokens it must carry, and it matches any simple command in the line,
 * through wrappers, prefixes and subshells, but never across two commands.
 */
import { expect, test } from "bun:test";
import { analyzeShell } from "../shell-parse.ts";
import { matchesRule } from "../shell-rules.ts";

const hits = (source: string, rule: string) =>
  analyzeShell(source).commands.some((command) => matchesRule(command, rule));

test.concurrent("a multi-word rule matches the command wherever it sits in the line", () => {
  for (const source of [
    "node -e 1",
    "cd /tmp && node -e 1",
    "FOO=1 node -e 1",
    "(node -e 1)",
    "sudo node -e 1",
    "/usr/local/bin/node -e 1",
    "bash -lc 'node -e 1'",
    'eval "node -e 1"',
  ]) {
    expect([source, hits(source, "node -e")]).toEqual([source, true]);
  }
});

test.concurrent("a rule does not match prose, a different command, or two commands together", () => {
  for (const source of [
    'git commit -m "fix node -e failure"',
    "node script.js",
    "mynode -e 1",
    "node script.js && echo -e x",
    "echo node -e",
  ]) {
    expect([source, hits(source, "node -e")]).toEqual([source, false]);
  }
  expect(hits('bun install && node -e "1"', "bun -e")).toBe(false);
  expect(hits("cd crate && cargo test", "cargo test")).toBe(true);
  expect(hits("cargo --verbose test", "cargo test")).toBe(true);
});

test.concurrent("a single-token rule names the command; a wrapper can be named too", () => {
  expect(hits("biome check .", "biome")).toBe(true);
  expect(hits("ls -la", "ls")).toBe(true);
  expect(hits("false; echo also", "ls")).toBe(false);
  expect(hits("sudo rm -rf x", "sudo")).toBe(true);
  expect(hits("sudo rm -rf x", "rm -rf")).toBe(true);
  expect(hits("./tools/x/check.sh", "tools/x/check.sh")).toBe(false);
  expect(hits("tools/x/check.sh --fix", "tools/x/check.sh")).toBe(true);
});

test.concurrent("an empty rule matches nothing", () => {
  expect(hits("ls", "")).toBe(false);
  expect(hits("ls", "   ")).toBe(false);
});
