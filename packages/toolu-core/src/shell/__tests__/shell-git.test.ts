/**
 * Git operations (#284 AC-7): the subcommand past value-taking global options,
 * a tristate that never turns a dynamic name into "not a push", the `-C` chain
 * and refspec destination of a push, and the static messages of a commit.
 */
import { expect, test } from "bun:test";
import { commitMessages, gitInvocation, pushTargets, runsGitSubcommand } from "../shell-git.ts";
import { analyzeShell } from "../shell-parse.ts";

const push = (source: string) => runsGitSubcommand(analyzeShell(source), "push");

test.concurrent("a push behind global options, a path or a line continuation is a push", () => {
  for (const source of [
    "git push",
    "/usr/bin/git push",
    "git --git-dir .git push",
    "git --work-tree=. push",
    "git --attr-source HEAD push",
    "git -c push.default=simple push",
    "git -C a -C b push",
    "git --no-pager -p push",
    "git \\\n  push",
  ]) {
    expect([source, push(source)]).toEqual([source, "yes"]);
  }
});

test.concurrent("words that only mention push are not a push", () => {
  for (const source of [
    'git commit -m "push"',
    "echo git push",
    'echo "no git push rules"',
    "git pushup",
    "gitpush",
    "git log --grep push",
    "cat <<EOF\ngit push\nEOF",
  ]) {
    expect([source, push(source)]).toEqual([source, "no"]);
  }
});

test.concurrent("a dynamic command name or subcommand is unknown, not 'not a push'", () => {
  for (const source of [
    "$g push",
    "git $(echo push)",
    "gi$(echo t) push",
    "git $SUB",
    "sudo $CMD",
    // bash globs the name or subcommand against the file system: `/usr/bin/g[i]t` runs git.
    "/usr/bin/g[i]t push",
    "git pu?h",
  ]) {
    expect([source, push(source)]).toEqual([source, "unknown"]);
  }
  expect(push(")")).toBe("unknown");
});

test.concurrent("a malformed or subcommand-less git invocation runs nothing", () => {
  for (const source of ["git -C", "git -c", "git -C dir", "git --version", "git"]) {
    const [command] = analyzeShell(source).commands;
    expect(command === undefined ? undefined : gitInvocation(command)).toBeUndefined();
    expect(push(source)).toBe("no");
  }
});

test.concurrent("the commit subcommand is exact", () => {
  expect(runsGitSubcommand(analyzeShell("git commit-tree abc"), "commit")).toBe("no");
  expect(runsGitSubcommand(analyzeShell("git -C /tmp/wt commit -m x"), "commit")).toBe("yes");
});

test.concurrent("a push reports its cumulative -C chain and refspec destination", () => {
  const [target] = pushTargets(
    analyzeShell('git -C "/tmp/my wt" -C inner push origin +HEAD:refs/heads/feat/x'),
  );
  expect([target?.cChain, target?.refspec, target?.destination]).toEqual([
    ["/tmp/my wt", "inner"],
    "+HEAD:refs/heads/feat/x",
    "feat/x",
  ]);
  const [second] = pushTargets(analyzeShell("git -C a log --grep push; git -C b push"));
  expect(second?.cChain).toEqual(["b"]);
});

test.concurrent("the destination follows the push_target_branch contract", () => {
  const destination = (source: string) => pushTargets(analyzeShell(source))[0]?.destination;
  const cases: readonly [string, string | null][] = [
    ["git push origin HEAD:feat/x", "feat/x"],
    ["git push -u origin feat/x", "feat/x"],
    ["git push origin abc123:feat/x", "feat/x"],
    ["git push -o ci.skip origin HEAD:feat/x", "feat/x"],
    ["git push --push-option=ci.skip origin HEAD:feat/x", "feat/x"],
    ["git push --repo origin origin HEAD:feat/x", "feat/x"],
    ["git push origin HEAD:feat/x 2>&1 | tee log", "feat/x"],
    ["git push", null],
    ["git push origin", null],
    ["git push origin HEAD", null],
    ["git push origin :feat/x", null],
    ["git push origin 'refs/heads/*:refs/heads/*'", null],
    ["git push origin $BRANCH", null],
  ];
  for (const [source, expected] of cases)
    expect([source, destination(source)]).toEqual([source, expected]);
});

test.concurrent("commit messages are read in every form, including the canonical heredoc", () => {
  const messages = (source: string) => {
    const commit = analyzeShell(source)
      .commands.map(gitInvocation)
      .find((g) => g?.subcommand === "commit");
    return commit === undefined ? [] : commitMessages(commit);
  };
  expect(messages(`git commit -m "$(cat <<'EOF'\nfeat: x\n\nbody\nEOF\n)"`)).toEqual([
    "feat: x\n\nbody",
  ]);
  expect(messages('git commit -am "fix: y"')).toEqual(["fix: y"]);
  expect(messages("git commit -m'chore: z'")).toEqual(["chore: z"]);
  expect(messages('git commit --message="docs: w"')).toEqual(["docs: w"]);
  expect(messages("git commit --message docs: -m second")).toEqual(["docs:", "second"]);
  expect(messages('git commit -m "$MSG"')).toEqual([null]);
  expect(messages("git commit -F msg.txt")).toEqual([]);
  expect(messages('git add -A && git commit -m "feat: x"')).toEqual(["feat: x"]);
});

test.concurrent("a push before a syntax error is still a push (AC-7)", () => {
  const analysis = analyzeShell('git push; echo "unterminated');
  expect(analysis.errors.length).toBeGreaterThan(0);
  expect(runsGitSubcommand(analysis, "push")).toBe("yes");
  expect(push("git push\necho $(")).toBe("yes");
});
