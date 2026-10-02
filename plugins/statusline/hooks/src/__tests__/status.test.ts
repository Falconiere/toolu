/** The Codex status report bundle against real repos and gate files (ported from status.bats). */
import { expect, test } from "bun:test";
import { realpathSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { git, put, report, repo, STATUS, withRemote } from "./harness.ts";

test.concurrent("status: reads Codex gate state and repository status without Claude fallback", async () => {
  using sb = createSandbox();
  repo(sb.project);
  sb.write(
    ".codex/tmp/quality-gate-status.json",
    '{"status":"failing","reason":"codex failure"}\n',
  );
  sb.write(".claude/tmp/quality-gate-status.json", '{"status":"passing"}\n');
  git(sb.project, "add", ".codex", ".claude");
  git(sb.project, "commit", "-q", "-m", "state");
  sb.write("untracked.txt", "");
  expect(await report(sb, sb.project, { TOOLU_HOST_OVERRIDE: "codex" })).toBe(
    [
      "Host: Codex",
      `Repository: ${realpathSync(sb.project)}`,
      "Branch: main",
      "Working tree: staged 0, unstaged 0, untracked 1",
      "Quality gate: failing — codex failure",
      "",
    ].join("\n"),
  );
});

test.concurrent("status: OpenCode report reads its own gate state", async () => {
  using sb = createSandbox();
  repo(sb.project);
  sb.write(
    ".opencode/tmp/quality-gate-status.json",
    '{"status":"failing","reason":"opencode failure"}\n',
  );
  sb.write(".codex/tmp/quality-gate-status.json", '{"status":"passing"}\n');
  const out = await report(sb, sb.project, { TOOLU_HOST_OVERRIDE: "opencode" });
  expect(out).toContain("Host: OpenCode\n");
  expect(out).toContain("Quality gate: failing — opencode failure\n");
});

test.concurrent("status: unknown explicit host override fails with a diagnostic", async () => {
  using sb = createSandbox();
  const res = Bun.spawnSync([process.execPath, STATUS, sb.project], {
    env: { ...process.env, HOME: sb.home, TOOLU_HOST_OVERRIDE: "opencodee" },
    stdout: "pipe",
    stderr: "pipe",
  });
  expect(res.exitCode).not.toBe(0);
  expect(res.stderr.toString()).toContain("unsupported statusline host override: opencodee");
});

test.concurrent("status: reads the host-native comemory marker when present", async () => {
  using sb = createSandbox();
  repo(sb.project);
  put(join(sb.codexHome, "comemory-status/project.json"), '{"repo":"project","count":7}\n');
  expect(await report(sb, sb.project)).toContain("Comemory: 7 memories\n");
});

test.concurrent("status: Codex report omits unavailable Claude model context and account fields", async () => {
  using sb = createSandbox();
  repo(sb.project);
  const out = await report(sb, sb.project);
  expect(out).toContain("Host: Codex\n");
  expect(out).toContain("Branch: main\n");
  expect(out).toContain("Quality gate: no recorded state\n");
  expect(out).not.toContain("Model:");
  expect(out).not.toContain("Context:");
  expect(out).not.toContain("Account:");
});

test.concurrent("status: selects Codex paths without lifecycle environment variables", async () => {
  using sb = createSandbox();
  sb.write(".codex/tmp/quality-gate-status.json", '{"status":"failing","reason":"codex state"}\n');
  sb.write(".claude/tmp/quality-gate-status.json", '{"status":"passing"}\n');
  const out = await report(sb, sb.project, { CODEX_HOME: undefined });
  expect(out).toContain("Host: Codex\n");
  expect(out).toContain("Quality gate: failing — codex state\n");
});

test.concurrent("status: a detached HEAD and a passing gate", async () => {
  using sb = createSandbox();
  repo(sb.project);
  git(sb.project, "checkout", "-q", "--detach");
  sb.write(".codex/tmp/quality-gate-status.json", '{"status":"passing"}');
  expect(await report(sb, sb.project)).toBe(
    [
      "Host: Codex",
      `Repository: ${realpathSync(sb.project)}`,
      "Branch: detached HEAD",
      "Working tree: staged 0, unstaged 0, untracked 1",
      "Quality gate: passing",
      "",
    ].join("\n"),
  );
});

test.concurrent("status: a folder outside git", async () => {
  using sb = createSandbox();
  sb.write(".codex/tmp/quality-gate-status.json", '{"status":"failing"}');
  expect(await report(sb, sb.project)).toBe(
    "Host: Codex\nFolder: project (not a git repository)\nQuality gate: failing\n",
  );
});

test.concurrent("status: ahead and behind its upstream", async () => {
  using sb = createSandbox();
  const dir = withRemote(sb);
  git(dir, "commit", "-q", "--allow-empty", "-m", "remote");
  git(dir, "push", "-q", "origin", "main");
  git(dir, "reset", "-q", "--hard", "HEAD~1");
  git(dir, "commit", "-q", "--allow-empty", "-m", "local");
  expect(await report(sb, dir)).toContain(
    "Branch: main (ahead 1) (behind 1)\nWorking tree: clean\n",
  );
});
