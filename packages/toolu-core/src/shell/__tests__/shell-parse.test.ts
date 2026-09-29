/**
 * `analyzeShell` on malformed, empty and oversize input (#284 AC-7): the
 * commands unbash could read are reported, errors come from every nested
 * script, and a line with no command at all is unknown.
 */
import { expect, test } from "bun:test";
import { analyzeShell, MAX_SHELL_INPUT } from "../shell-parse.ts";

const argvs = (source: string) => analyzeShell(source).commands.map((c) => c.argv);

test.concurrent("a valid command followed by a syntax error is still reported", () => {
  const analysis = analyzeShell('git push; echo "unterminated');
  expect(analysis.commands.map((c) => c.argv)).toContainEqual(["git", "push"]);
  expect(analysis.errors.length).toBeGreaterThan(0);
  expect(analysis.errors[0]?.message).toBe("unterminated double quote");
  expect(analysis.unknown).toBe(false);
});

test.concurrent("a line bash cannot parse proves nothing through its exit status", () => {
  const analysis = analyzeShell("bun test &&");
  expect(analysis.commands.map((c) => [c.argv, c.exitProves])).toEqual([[["bun", "test"], false]]);
  expect(analyzeShell("bun test").commands[0]?.exitProves).toBe(true);
});

test.concurrent("errors with no command at all make the line unknown", () => {
  for (const source of [")", "if", "fi"]) {
    const analysis = analyzeShell(source);
    expect(analysis.commands).toEqual([]);
    expect(analysis.unknown).toBe(true);
  }
});

test.concurrent("errors inside a substitution are collected from the nested script", () => {
  const analysis = analyzeShell('echo $(git push; echo "oops)');
  expect(analysis.errors.map((e) => [e.message, e.origin])).toEqual([
    ["unterminated command substitution", "line"],
    ["unterminated double quote", "substitution"],
  ]);
  expect(analysis.commands.map((c) => c.argv)).toContainEqual(["git", "push"]);
});

test.concurrent("empty and whitespace-only input runs nothing and is not unknown", () => {
  for (const source of ["", "   ", "\n\t\n"]) {
    expect(analyzeShell(source)).toEqual({
      source,
      commands: [],
      compoundRedirects: [],
      errors: [],
      unknown: false,
    });
  }
});

test.concurrent("input over the cap is not parsed and is unknown", () => {
  const source = `echo ${"x".repeat(MAX_SHELL_INPUT)}`;
  const analysis = analyzeShell(source);
  expect(analysis.unknown).toBe(true);
  expect(analysis.commands).toEqual([]);
  expect(analysis.errors[0]?.message).toStartWith("oversize:");
});

test.concurrent("input at the cap is parsed", () => {
  const body = "x".repeat(MAX_SHELL_INPUT - "echo ".length);
  const analysis = analyzeShell(`echo ${body}`);
  expect(analysis.unknown).toBe(false);
  expect(analysis.commands[0]?.argv[0]).toBe("echo");
});

test.concurrent("a heredoc body several hundred KiB long stays analyzable", () => {
  const body = "const x = 1; // not a command\n".repeat(20_000);
  expect(argvs(`cat > big.ts <<'EOF'\n${body}EOF\ngit add big.ts`)).toEqual([
    ["cat"],
    ["git", "add", "big.ts"],
  ]);
});
