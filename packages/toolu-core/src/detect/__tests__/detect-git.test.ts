/**
 * Git questions on `@toolu/core/shell` (#254 AC-1, AC-2, AC-9). The
 * bats-parity and #283 fixtures run through these same functions in
 * `shell/__tests__`. Here, the five `strip_heredocs` bats inputs are replayed
 * as commands whose heredoc body holds a push and a redirect, against the
 * unmodified `is_git_push`, and the unknown-to-false mapping is pinned.
 */
import { expect, test } from "bun:test";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { analyzeShell, runsGitSubcommand } from "../../shell/shell.ts";
import { writeTargets } from "../../shell/shell-writes.ts";
import { isGitCommit, isGitPush, pushTargetBranch, pushTargetRoot } from "../detect-git.ts";
import { bashDetect, detectEnv } from "./detect-bash.ts";

const lines = (...parts: string[]) => parts.join("\n");

/** Each bats input, with a push and a redirect planted in the heredoc body. */
const HEREDOC_CASES = [
  {
    name: "<<EOF > /tmp/x body is stripped, trailing command preserved",
    command: lines(
      "cat <<EOF > /tmp/x",
      "git push origin main",
      "echo s > body.env",
      "EOF",
      "echo after",
    ),
    push: false,
    targets: ["/tmp/x"],
  },
  {
    name: "<<-END (tab-indented form) is stripped",
    command: lines("cat <<-END", "\tgit push", "\techo s > body.env", "\tEND", "echo end"),
    push: false,
    targets: [],
  },
  {
    name: "<<DOC alternate delimiter is stripped",
    command: lines("cat <<DOC", "git push", "y", "DOC", "echo end"),
    push: false,
    targets: [],
  },
  {
    name: "plain command (no heredoc) passes through unchanged",
    command: lines("echo hello", "ls -la", "git push"),
    push: true,
    targets: [],
  },
  {
    name: "<<EOF | tee (pipe after heredoc start) strips body",
    command: lines(
      "cat <<EOF | tee /tmp/x",
      "secret git push inside body > body.env",
      "EOF",
      "echo done",
    ),
    push: false,
    targets: ["/tmp/x"],
  },
] as const;

test.concurrent.each(HEREDOC_CASES.map((c) => [c.name, c] as const))(
  "strip_heredocs %s: the body runs nothing, the lines around it do",
  async (_name, c) => {
    using sb = createSandbox();
    const analysis = analyzeShell(c.command);
    const bash = await bashDetect(
      'if is_git_push "$1"; then printf yes; else printf no; fi',
      [c.command],
      sb.project,
      detectEnv(sb.home),
    );
    expect(isGitPush(analysis)).toBe(bash === "yes");
    expect(isGitPush(analysis)).toBe(c.push);
    expect(writeTargets(analysis).map((t) => t.path)).toEqual([...c.targets]);
    const names = analysis.commands.map((cmd) => cmd.argv[0]);
    expect(names).toContain("echo");
    expect(names).not.toContain("secret");
  },
);

test("a dynamic subcommand is unknown to the shell layer and false here, as bash answers", async () => {
  using sb = createSandbox();
  for (const command of ["$g push", "git $(echo push)", '"$GIT" commit -m x']) {
    const analysis = analyzeShell(command);
    expect(
      runsGitSubcommand(analysis, "push") === "unknown" ||
        runsGitSubcommand(analysis, "commit") === "unknown",
    ).toBe(true);
    const bash = await bashDetect(
      'if is_git_push "$1" || is_git_commit "$1"; then printf yes; else printf no; fi',
      [command],
      sb.project,
      detectEnv(sb.home),
    );
    expect(bash).toBe("no");
    expect(isGitPush(analysis) || isGitCommit(analysis)).toBe(false);
  }
});

test("pushTargetRoot resolves a -C path with spaces; pushTargetBranch reads a detached refspec", () => {
  using sb = createSandbox({ git: true, branch: "feat/x" });
  sb.git("worktree", "add", "-q", "--detach", sb.path("wt with space"));
  const env = detectEnv(sb.home);
  const wt = sb.path("wt with space");
  const analysis = analyzeShell(`git -C "${wt}" push origin HEAD:feat/y`);
  expect(pushTargetRoot(analysis, { env, cwd: sb.project })).toBe(wt);
  expect(pushTargetBranch(analysis, wt, env)).toBe("feat/y");
  expect(pushTargetBranch(analysis, sb.project, env)).toBe("feat/x");
  expect(pushTargetBranch(analyzeShell("git push origin :gone"), wt, env)).toBe("");
  expect(pushTargetRoot(analyzeShell('git -C "$D" push'), { env, cwd: sb.project })).toBe(
    sb.project,
  );
});
