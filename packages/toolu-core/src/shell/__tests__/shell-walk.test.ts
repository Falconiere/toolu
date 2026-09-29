/**
 * The exhaustive walk (#284 AC-8, AC-10): every construct that runs a command
 * yields it with the right origin, quoted heredoc bodies stay data, compound
 * redirects are reported, and the exit-status rule holds per position. The
 * source scan pins "no `as`, no hand-written type guards" over parser output.
 */
import { expect, test } from "bun:test";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import ts from "typescript";
import { analyzeShell } from "../shell-parse.ts";
import type { CommandOrigin } from "../shell-types.ts";

const pushes = (source: string) =>
  analyzeShell(source)
    .commands.filter((c) => c.argv[0] === "git" && c.argv[1] === "push")
    .map((c) => c.origin);

const CONSTRUCTS: readonly [string, string, CommandOrigin][] = [
  ["command substitution", "echo $(git push)", "substitution"],
  ["backticks", "echo `git push`", "substitution"],
  ["input process substitution", "diff <(git push) x", "substitution"],
  ["output process substitution", "echo x | tee >(git push)", "substitution"],
  ["subshell", "(cd x && git push)", "line"],
  ["brace group", "{ git push; }", "line"],
  ["case arm", "case $x in a) git push;; esac", "line"],
  ["function body", "deploy() { git push; }", "function"],
  ["[[ ]] operand", "[[ -n $(git push) ]]", "substitution"],
  ["arithmetic command", "(( $(git push) + 1 ))", "substitution"],
  ["arithmetic expansion", "echo $(( $(git push) + 1 ))", "substitution"],
  ["parameter operand", "echo ${X:-$(git push)}", "substitution"],
  ["assignment value", "x=$(git push)", "substitution"],
  ["redirect target", "echo hi > $(git push)", "substitution"],
  ["unquoted heredoc body", "cat <<EOF\n$(git push)\nEOF", "substitution"],
  ["if condition", "if git push; then :; fi", "line"],
  ["while body", "while true; do git push; done", "line"],
  ["for word list", "for x in $(git push); do :; done", "substitution"],
];

for (const [name, source, origin] of CONSTRUCTS) {
  test.concurrent(`reports a command inside ${name}`, () => {
    expect(pushes(source)).toEqual([origin]);
  });
}

test.concurrent("a quoted heredoc body is data, not a command", () => {
  expect(pushes("cat <<'EOF'\n$(git push)\nEOF")).toEqual([]);
  expect(pushes('cat <<"EOF"\ngit push\nEOF')).toEqual([]);
  expect(pushes("cat <<EOF\ngit push\nEOF")).toEqual([]);
});

test.concurrent("redirects on compound commands are reported as compound redirects", () => {
  const targets = (source: string) => analyzeShell(source).compoundRedirects.map((r) => r.target);
  expect(targets("{ echo x; } >.env")).toEqual([".env"]);
  expect(targets("(echo x) >> .env")).toEqual([".env"]);
  expect(targets("for i in 1; do echo $i; done > out.txt")).toEqual(["out.txt"]);
  expect(targets("f() { echo x; } > log")).toEqual(["log"]);
  expect(targets("echo x > .env")).toEqual([]);
});

test.concurrent("pipeline position and exit observability follow bash semantics", () => {
  const view = (source: string) =>
    analyzeShell(source).commands.map((c) => [c.argv[0], c.pipeline.index, c.exitProves]);
  expect(view("bun test 2>&1 | tail -20")).toEqual([
    ["bun", 0, false],
    ["tail", 1, true],
  ]);
  expect(view("cd x && bun test")).toEqual([
    ["cd", 0, true],
    ["bun", 0, true],
  ]);
  expect(view("bun test || true")).toEqual([
    ["bun", 0, false],
    ["true", 0, false],
  ]);
  expect(view("bun test; echo done")).toEqual([
    ["bun", 0, false],
    ["echo", 0, true],
  ]);
  expect(view("bun test &")).toEqual([["bun", 0, false]]);
  expect(view("! bun test")).toEqual([["bun", 0, false]]);
  expect(view("a || b && bun test")).toEqual([
    ["a", 0, false],
    ["b", 0, false],
    ["bun", 0, true],
  ]);
  expect(view("(bun test)")).toEqual([["bun", 0, true]]);
  expect(view("if bun test; then echo ok; fi")).toEqual([
    ["bun", 0, false],
    ["echo", 0, false],
  ]);
  expect(view("echo $(bun test)")).toEqual([
    ["bun", 0, false],
    ["echo", 0, true],
  ]);
});

test.concurrent("words keep static values and mark expansions dynamic", () => {
  const { commands } = analyzeShell(`echo 'a b' "c\\"d" $'e\\tf' $HOME "$(date)" x*.ts`);
  expect(commands.map((c) => c.argv[0])).toEqual(["date", "echo"]);
  expect(commands[1]?.words).toEqual(["echo", "a b", 'c"d', "e\tf", null, null, "x*.ts"]);
});

test.concurrent("a heredoc piped through cat in double quotes resolves to its body", () => {
  const message = "feat(core): x\n\nBody line with $HOME and `ticks`.";
  const source = `git commit -m "$(cat <<'EOF'\n${message}\nEOF\n)"`;
  expect(analyzeShell(source).commands.at(-1)?.words).toEqual(["git", "commit", "-m", message]);
  const tabbed = `git commit -m "$(cat <<-EOF\n\tfeat: y\n\tEOF\n)"`;
  expect(analyzeShell(tabbed).commands.at(-1)?.words.at(-1)).toBe("feat: y");
  const expanding = `git commit -m "$(cat <<EOF\nfeat: $X\nEOF\n)"`;
  expect(analyzeShell(expanding).commands.at(-1)?.words.at(-1)).toBeNull();
});

/** AC-8: parser output is narrowed by `switch`, never asserted or guarded by hand. */
test.concurrent("shell sources contain no type assertions, non-null assertions or type predicates", () => {
  const dir = join(import.meta.dir, "..");
  const found: string[] = [];
  for (const file of readdirSync(dir).filter((name) => name.endsWith(".ts"))) {
    const text = readFileSync(join(dir, file), "utf8");
    const source = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true);
    const visit = (node: ts.Node): void => {
      const banned =
        ts.isAsExpression(node) ||
        ts.isTypeAssertionExpression(node) ||
        ts.isNonNullExpression(node) ||
        ts.isTypePredicateNode(node);
      if (banned) found.push(`${file}: ${node.getText(source)}`);
      ts.forEachChild(node, visit);
    };
    visit(source);
  }
  expect(found).toEqual([]);
});
