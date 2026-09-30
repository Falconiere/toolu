/** The Claude Code statusline bundle with real JSON payloads on stdin and real repos (ported from statusline.bats). */
import { expect, test } from "bun:test";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import {
  cfgOf,
  git,
  payload,
  plain,
  publishJev,
  put,
  render,
  repo,
  withRemote,
} from "./harness.ts";

const SEP = "\x1b[2m | \x1b[0m";
const CTX_ONLY = `\x1b[36mClaude\x1b[0m${SEP}\x1b[35mctx:0/0\x1b[0m`;

function ctxPayload(tokens: number, pct: number): string {
  return `{"model":{"display_name":"Opus"},"context_window":{"context_window_size":200000,"total_input_tokens":${tokens},"used_percentage":${pct}}}`;
}

test.concurrent("statusline: renders model and context segments", async () => {
  using sb = createSandbox();
  const out = plain(await render(sb, { payload: ctxPayload(45000, 22) }));
  expect(out).toContain("Opus");
  expect(out).toContain("ctx:45k/200k (22%)");
});

test.concurrent("statusline: effort shown when present", async () => {
  using sb = createSandbox();
  const out = await render(sb, { payload: payload(sb.project, ',"effort":{"level":"high"}') });
  expect(plain(out)).toContain("effort:high");
});

test.concurrent("statusline: effort omitted when absent", async () => {
  using sb = createSandbox();
  expect(plain(await render(sb))).not.toContain("effort:");
});

test.concurrent("statusline: red gate marker when the project gate is failing", async () => {
  using sb = createSandbox();
  sb.write(".claude/tmp/quality-gate-status.json", '{"status":"failing","reason":"x"}');
  expect(await render(sb)).toContain("\x1b[1m\x1b[31m✗ gate:failing\x1b[0m");
});

test.concurrent("statusline: no gate marker when the gate is passing", async () => {
  using sb = createSandbox();
  sb.write(".claude/tmp/quality-gate-status.json", '{"status":"passing"}');
  expect(plain(await render(sb))).not.toContain("gate:failing");
});

test.concurrent("statusline: no gate marker when there is no gate file", async () => {
  using sb = createSandbox();
  expect(plain(await render(sb))).not.toContain("gate:failing");
});

test.concurrent("statusline: branch + folder shown for a git workspace", async () => {
  using sb = createSandbox();
  repo(sb.project);
  expect(plain(await render(sb))).toBe("Opus | ctx:1k/200k | project | main");
});

test.concurrent("statusline: dirty shows staged files with [+N]", async () => {
  using sb = createSandbox();
  repo(sb.project);
  sb.write("staged.txt", "");
  git(sb.project, "add", "staged.txt");
  expect(plain(await render(sb))).toContain("[+1]");
});

test.concurrent("statusline: dirty shows unstaged files with [~N]", async () => {
  using sb = createSandbox();
  repo(sb.project);
  sb.write("f.txt", "c\n");
  git(sb.project, "add", "f.txt");
  git(sb.project, "commit", "-qm", "add");
  sb.write("f.txt", "c\nch\n");
  expect(plain(await render(sb))).toContain("[~1]");
});

test.concurrent("statusline: dirty shows untracked files with [?N]", async () => {
  using sb = createSandbox();
  repo(sb.project);
  sb.write("u.txt", "");
  expect(plain(await render(sb))).toContain("[?1]");
});

test.concurrent("statusline: dirty shows all three counts when mixed", async () => {
  using sb = createSandbox();
  repo(sb.project);
  sb.write("f.txt", "c\n");
  git(sb.project, "add", "f.txt");
  git(sb.project, "commit", "-qm", "add");
  sb.write("f.txt", "c\nch\n");
  sb.write("s.txt", "");
  git(sb.project, "add", "s.txt");
  sb.write("u.txt", "");
  expect(await render(sb)).toEndWith(
    `${SEP}\x1b[1mproject\x1b[0m${SEP}\x1b[34mmain\x1b[0m\x1b[33m[+1 ~1 ?1]\x1b[0m`,
  );
});

test.concurrent("statusline: clean repo omits dirty bracket", async () => {
  using sb = createSandbox();
  repo(sb.project);
  expect(plain(await render(sb))).not.toMatch(/\[[+~?]/);
});

test.concurrent("statusline: ahead arrow shown when local is ahead of upstream", async () => {
  using sb = createSandbox();
  const dir = withRemote(sb);
  git(dir, "commit", "-q", "--allow-empty", "-m", "ahead");
  expect(plain(await render(sb, { payload: payload(dir) }))).toContain("main↑1");
});

test.concurrent("statusline: behind arrow shown when local is behind upstream", async () => {
  using sb = createSandbox();
  const dir = withRemote(sb);
  git(dir, "commit", "-q", "--allow-empty", "-m", "behind");
  git(dir, "push", "-q", "origin", "main");
  git(dir, "reset", "-q", "--hard", "HEAD~1");
  expect(plain(await render(sb, { payload: payload(dir) }))).toContain("main↓1");
});

test.concurrent("statusline: dirty and ahead shown together", async () => {
  using sb = createSandbox();
  const dir = withRemote(sb);
  git(dir, "commit", "-q", "--allow-empty", "-m", "ahead");
  put(join(dir, "new.txt"), "");
  expect(await render(sb, { payload: payload(dir) })).toEndWith(
    "\x1b[34mmain\x1b[0m\x1b[2m↑1\x1b[0m\x1b[33m[?1]\x1b[0m",
  );
});

test.concurrent("statusline: no dirty status outside a git repo", async () => {
  using sb = createSandbox();
  sb.write("u.txt", "");
  expect(plain(await render(sb))).toBe("Opus | ctx:1k/200k | project");
});

test.concurrent("statusline: gate marker resolves via git root when cwd is a subdir", async () => {
  using sb = createSandbox();
  repo(sb.project);
  sb.write(".claude/tmp/quality-gate-status.json", '{"status":"failing","reason":"x"}');
  mkdirSync(sb.path("packages/app/src"), { recursive: true });
  const out = plain(await render(sb, { payload: payload(sb.path("packages/app/src")) }));
  expect(out).toBe("Opus | ctx:1k/200k | ✗ gate:failing | src | main[?1]");
});

test.concurrent("statusline: format_tokens M-tier renders millions with one decimal", async () => {
  using sb = createSandbox();
  expect(plain(await render(sb, { payload: ctxPayload(13779513, 99) }))).toContain(
    "ctx:13.7M/200k",
  );
});

test.concurrent("statusline: format_tokens M-tier stays k below a million", async () => {
  using sb = createSandbox();
  expect(plain(await render(sb, { payload: ctxPayload(13779, 7) }))).toContain("ctx:13k/200k (7%)");
});

test.concurrent("statusline: account:renders the email domain from .claude.json", async () => {
  using sb = createSandbox();
  put(join(cfgOf(sb), ".claude.json"), '{"oauthAccount":{"emailAddress":"hello@example.com"}}');
  const out = plain(await render(sb, { payload: ctxPayload(1000, 1) }));
  expect(out).toContain("ctx:1k/200k (1%) | example.com");
  expect(out).not.toContain("hello@example.com");
});

test.concurrent("statusline: account:omitted when .claude.json has no oauthAccount", async () => {
  using sb = createSandbox();
  put(join(cfgOf(sb), ".claude.json"), "{}");
  expect(plain(await render(sb, { payload: ctxPayload(1000, 1) }))).toBe("Opus | ctx:1k/200k (1%)");
});

test.concurrent("statusline: account:a custom CLAUDE_CONFIG_DIR without .claude.json never falls back to $HOME", async () => {
  using sb = createSandbox();
  put(
    join(sb.home, ".claude.json"),
    '{"oauthAccount":{"emailAddress":"hello@shouldnotleak.example"}}',
  );
  expect(plain(await render(sb, { payload: ctxPayload(1000, 1) }))).toBe("Opus | ctx:1k/200k (1%)");
});

test.concurrent("statusline: account:read from $HOME when CLAUDE_CONFIG_DIR is unset", async () => {
  using sb = createSandbox();
  put(join(sb.home, ".claude.json"), '{"oauthAccount":{"emailAddress":"a@home.example"}}');
  const out = await render(sb, {
    payload: ctxPayload(1000, 1),
    env: { CLAUDE_CONFIG_DIR: undefined },
  });
  expect(plain(out)).toBe("Opus | ctx:1k/200k (1%) | home.example");
});

test.concurrent("statusline: comemory:renders the count from the comemory marker", async () => {
  using sb = createSandbox();
  repo(sb.project);
  put(join(cfgOf(sb), "comemory-status/project.json"), '{"repo":"project","count":7}');
  expect(await render(sb)).toContain("\x1b[1m\x1b[32m[COMEMORY:7]\x1b[0m");
});

test.concurrent("statusline: comemory:worktree resolves to the main-repo key", async () => {
  using sb = createSandbox();
  const main = repo(join(sb.root, "main"));
  git(main, "worktree", "add", "-q", join(sb.root, "wt"));
  put(join(cfgOf(sb), "comemory-status/main.json"), '{"repo":"main","count":5}');
  const out = plain(await render(sb, { payload: payload(join(sb.root, "wt")) }));
  expect(out).toContain("[COMEMORY:5]");
});

test.concurrent("statusline: comemory:omitted when there is no marker", async () => {
  using sb = createSandbox();
  repo(sb.project);
  expect(plain(await render(sb))).not.toContain("[COMEMORY:");
});

test.concurrent("statusline: every segment in order, colours included", async () => {
  using sb = createSandbox();
  const dir = withRemote(sb);
  git(dir, "commit", "-q", "--allow-empty", "-m", "ahead");
  put(join(dir, "u.txt"), "");
  put(join(dir, ".claude/tmp/quality-gate-status.json"), '{"status":"failing"}');
  put(join(cfgOf(sb), ".claude.json"), '{"oauthAccount":{"emailAddress":"me@corp.example"}}');
  put(join(cfgOf(sb), "comemory-status/repo.json"), '{"count":42}');
  await publishJev(cfgOf(sb), "claude");
  const out = await render(sb, {
    payload: `{"model":{"display_name":"Opus 4"},"effort":{"level":"high"},"workspace":{"current_dir":${JSON.stringify(dir)}},"context_window":{"context_window_size":1000000,"total_input_tokens":456789,"used_percentage":45.6}}`,
  });
  expect(out).toBe(
    [
      "\x1b[36mOpus 4\x1b[0m",
      "\x1b[33meffort:high\x1b[0m",
      "\x1b[35mctx:456k/1.0M (46%)\x1b[0m",
      "\x1b[32mcorp.example\x1b[0m",
      "\x1b[1m\x1b[31m✗ gate:failing\x1b[0m",
      "\x1b[1mrepo\x1b[0m",
      "\x1b[34mmain\x1b[0m\x1b[2m↑1\x1b[0m\x1b[33m[?2]\x1b[0m",
      "\x1b[1m\x1b[32m[COMEMORY:42]\x1b[0m",
      "\x1b[1m\x1b[32m[JEV:READY]\x1b[0m",
    ].join(SEP),
  );
});

test.concurrent("statusline: a malformed payload still renders the defaults", async () => {
  using sb = createSandbox();
  for (const body of ["not json", "", "[1,2]", '"str"', "null"]) {
    expect(await render(sb, { payload: body })).toBe(CTX_ONLY);
  }
});

test.concurrent("statusline: a wrongly typed field stops reading the payload there", async () => {
  using sb = createSandbox();
  const modelString = `{"model":"Opus","workspace":{"current_dir":${JSON.stringify(sb.project)}}}`;
  expect(await render(sb, { payload: modelString })).toBe(CTX_ONLY);
  const effortString = `{"model":{"display_name":"M"},"effort":"x","workspace":{"current_dir":${JSON.stringify(sb.project)}}}`;
  expect(plain(await render(sb, { payload: effortString }))).toBe("M | ctx:0/0");
});

test.concurrent("statusline: exact half percentages round to even", async () => {
  using sb = createSandbox();
  const pct = async (value: number) =>
    plain(await render(sb, { payload: ctxPayload(1000, value) }));
  expect(await pct(22.5)).toBe("Opus | ctx:1k/200k (22%)");
  expect(await pct(23.5)).toBe("Opus | ctx:1k/200k (24%)");
  expect(await pct(0.5)).toBe("Opus | ctx:1k/200k (0%)");
});

test.concurrent("statusline: a branch named like a tracking word keeps its counts", async () => {
  using sb = createSandbox();
  const dir = withRemote(sb, "ahead-behind");
  git(dir, "commit", "-q", "--allow-empty", "-m", "ahead");
  put(join(dir, "u.txt"), "");
  expect(plain(await render(sb, { payload: payload(dir) }))).toBe(
    "Opus | ctx:1k/200k | repo | ahead-behind↑1[?1]",
  );
});
