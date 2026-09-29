/**
 * What a command actually runs (#284): wrappers are peeled with their own
 * options, `command -v` runs nothing, xargs adds arguments read at run time,
 * and shell strings (`bash -c`, a heredoc fed to a shell, `eval`) are analyzed
 * to a fixed depth, with anything unreadable reported as an unknown command.
 */
import { expect, test } from "bun:test";
import { analyzeShell } from "../shell-parse.ts";
import { MAX_RUN_DEPTH } from "../shell-walk.ts";

const runs = (source: string) =>
  analyzeShell(source).commands.map((c) => ({ argv: c.argv, wrappers: c.wrappers }));

const WRAPPED: readonly [string, readonly string[]][] = [
  ["timeout 120 git push", ["timeout"]],
  ["timeout -s KILL -k 5 60 git push", ["timeout"]],
  ["nice -n 10 git push", ["nice"]],
  ["nice -10 git push", ["nice"]],
  ["sudo -u me git push", ["sudo"]],
  ["sudo -E VAR=1 git push", ["sudo"]],
  ["/usr/bin/sudo git push", ["sudo"]],
  ["doas -u me git push", ["doas"]],
  ["env -i PATH=/usr/bin git push", ["env"]],
  ["env -u HOME -C /tmp git push", ["env"]],
  ["env -P /usr/bin git push", ["env"]],
  ["env - git push", ["env"]],
  ["env -i - A=1 git push", ["env"]],
  ["command git push", ["command"]],
  ["builtin git push", ["builtin"]],
  ["exec -a name git push", ["exec"]],
  ["nohup git push", ["nohup"]],
  ["stdbuf -oL -e 0 git push", ["stdbuf"]],
  ["sudo -u me timeout 60 nice -n 5 git push", ["sudo", "timeout", "nice"]],
];

for (const [source, wrappers] of WRAPPED) {
  test.concurrent(`unwraps ${source}`, () => {
    expect(runs(source)).toEqual([{ argv: ["git", "push"], wrappers }]);
  });
}

test.concurrent("the words as written keep the wrappers", () => {
  expect(analyzeShell("sudo -u me git push").commands[0]?.words).toEqual([
    "sudo",
    "-u",
    "me",
    "git",
    "push",
  ]);
});

test.concurrent("xargs appends arguments read at run time", () => {
  expect(runs("echo main | xargs git push origin")).toEqual([
    { argv: ["echo", "main"], wrappers: [] },
    { argv: ["git", "push", "origin", null], wrappers: ["xargs"] },
  ]);
  expect(runs("xargs -I {} -n 1 git push origin {}").at(-1)?.argv).toEqual([
    "git",
    "push",
    "origin",
    "{}",
    null,
  ]);
});

test.concurrent("wrappers that run no command are not unwrapped", () => {
  for (const source of [
    "command -v git",
    "sudo -l",
    "sudo -e /etc/hosts",
    "env",
    "timeout 5",
    "exec 3>.env",
  ]) {
    expect(runs(source)[0]?.wrappers).toEqual([]);
  }
  expect(runs("command -v git")[0]?.argv).toEqual(["command", "-v", "git"]);
});

test.concurrent("a wrapper hiding its command in a string makes the command unknown", () => {
  expect(runs("env -S 'git push'")).toEqual([{ argv: [null], wrappers: ["env"] }]);
  expect(runs("sudo $CMD")).toEqual([{ argv: [null], wrappers: ["sudo"] }]);
});

test.concurrent("bash -c and friends are analyzed as their own command lines", () => {
  for (const source of [
    "bash -c 'git push'",
    'sh -c "git push"',
    "bash -lc 'git push'",
    "zsh -o pipefail -c 'git push'",
    "/bin/dash -e -c 'git push'",
    "ksh -c 'git push' arg0",
  ]) {
    const inner = analyzeShell(source).commands.at(-1);
    expect([inner?.argv, inner?.origin, inner?.depth]).toEqual([["git", "push"], "shell", 1]);
  }
});

test.concurrent("a static heredoc or herestring fed to a shell is code", () => {
  const heredoc = analyzeShell("bash <<'EOF'\ngit push\nEOF").commands.at(-1);
  expect([heredoc?.argv, heredoc?.origin]).toEqual([["git", "push"], "shell"]);
  expect(analyzeShell('bash <<< "git push"').commands.at(-1)?.argv).toEqual(["git", "push"]);
  expect(analyzeShell("cat <<'EOF'\ngit push\nEOF").commands.map((c) => c.argv)).toEqual([["cat"]]);
});

test.concurrent("eval joins its arguments into a command line", () => {
  const inner = analyzeShell('eval "git push" origin').commands.at(-1);
  expect([inner?.argv, inner?.origin]).toEqual([["git", "push", "origin"], "eval"]);
  expect(analyzeShell("eval -- git push").commands.at(-1)?.argv).toEqual(["git", "push"]);
});

test.concurrent("a shell string that cannot be read statically is an unknown command", () => {
  for (const source of ['bash -c "$CMD"', "eval $CMD", "curl -s x | bash", "sh -s"]) {
    const inner = analyzeShell(source).commands.at(-1);
    expect(inner?.argv).toEqual([null]);
  }
  expect(analyzeShell("bash deploy.sh").commands.map((c) => c.argv)).toEqual([
    ["bash", "deploy.sh"],
  ]);
});

test.concurrent("recursion stops at the depth limit with an unknown command", () => {
  let source = "git push";
  for (let depth = 0; depth < MAX_RUN_DEPTH; depth++) source = `bash -c ${JSON.stringify(source)}`;
  expect(analyzeShell(source).commands.at(-1)?.argv).toEqual(["git", "push"]);
  const deeper = analyzeShell(`bash -c ${JSON.stringify(source)}`).commands.at(-1);
  expect([deeper?.argv, deeper?.depth]).toEqual([[null], MAX_RUN_DEPTH + 1]);
});

test.concurrent("the exit status of a shell string is its last command's", () => {
  const proves = (source: string) => analyzeShell(source).commands.at(-1)?.exitProves;
  expect(proves("bash -c 'cd x && bun test'")).toBe(true);
  expect(proves("bash -c 'bun test | tail'")).toBe(true);
  expect(analyzeShell("bash -c 'bun test | tail'").commands.at(-2)?.exitProves).toBe(false);
  expect(proves("bash -c 'bun test' || true")).toBe(false);
});

test.concurrent("a command under xargs proves nothing through the exit status", () => {
  // `xargs -r` with empty input runs the command zero times and exits 0.
  const proves = (source: string) =>
    analyzeShell(source).commands.map((c) => [c.argv[0], c.exitProves]);
  expect(proves('printf "" | xargs -r bun test')).toEqual([
    ["printf", false],
    ["bun", false],
  ]);
  expect(proves("xargs bash -c 'bun test'").at(-1)).toEqual(["bun", false]);
});

test.concurrent("argv words keep their dequoted text, expansions as written", () => {
  const [command] = analyzeShell(`sudo cp "$HOME/a b" 'c d' e`).commands;
  expect(command?.argv).toEqual(["cp", null, "c d", "e"]);
  expect(command?.texts).toEqual(["cp", "$HOME/a b", "c d", "e"]);
});
