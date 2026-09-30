/**
 * Jev readiness through the real Jev publisher and both statusline consumers,
 * without API calls (ported from jev.bats). The statusline only reads the
 * active profile's published wrapper; it never loads the sibling plugin.
 */
import { expect, test } from "bun:test";
import { chmodSync, copyFileSync, mkdirSync, rmSync, symlinkSync } from "node:fs";
import { join } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import {
  JEV_BUNDLE,
  KEY,
  cfgOf,
  payload,
  plain,
  publishJev,
  put,
  render,
  report,
  repo,
} from "./harness.ts";

const SEP = "\x1b[2m | \x1b[0m";
const READY = "\x1b[1m\x1b[32m[JEV:READY]\x1b[0m";
const unavailable = (reason: string) => `\x1b[1m\x1b[33m[JEV:UNAVAILABLE: ${reason}]\x1b[0m`;

/** Claude's render of an empty workspace, the way jev.bats piped `{workspace:{current_dir}}`. */
function renderWorkspace(sb: Sandbox, env = {}): Promise<string> {
  mkdirSync(sb.path("workspace"), { recursive: true });
  return render(sb, {
    payload: JSON.stringify({ workspace: { current_dir: sb.path("workspace") } }),
    env,
  });
}

test.concurrent("Jev readiness: Claude renders ready from the published wrapper", async () => {
  using sb = createSandbox();
  await publishJev(cfgOf(sb), "claude");
  const out = await renderWorkspace(sb);
  expect(out).toEndWith(`${SEP}${READY}`);
  expect(out).not.toContain(KEY);
});

test.concurrent("Jev readiness: Codex report uses its published wrapper without lifecycle variables", async () => {
  using sb = createSandbox();
  await publishJev(sb.codexHome, "codex");
  const out = await report(sb, sb.project);
  expect(out).toContain("Jev: ready\n");
  expect(out).not.toContain(KEY);
});

test.concurrent("Jev readiness: missing or empty key is unavailable with a reason", async () => {
  using sb = createSandbox();
  await publishJev(cfgOf(sb), "claude");
  expect(await renderWorkspace(sb, { TYPESAFE_API_KEY: undefined })).toEndWith(
    unavailable("missing TYPESAFE_API_KEY"),
  );
  await publishJev(sb.codexHome, "codex");
  expect(await report(sb, sb.project, { TYPESAFE_API_KEY: "" })).toContain(
    "Jev: unavailable — missing TYPESAFE_API_KEY\n",
  );
});

test.concurrent("Jev readiness: malformed credentials are unavailable without exposing their contents", async () => {
  using sb = createSandbox();
  await publishJev(cfgOf(sb), "claude");
  for (const key of ["secret\nvalue", "secret\rvalue", "\r", "\n"]) {
    const out = await renderWorkspace(sb, { TYPESAFE_API_KEY: key });
    expect(out).toEndWith(unavailable("invalid TYPESAFE_API_KEY"));
    expect(out).not.toContain("secret");
    expect(out).not.toContain("value");
  }
});

test.concurrent("Jev readiness: unpublished plugin is omitted on both hosts", async () => {
  using sb = createSandbox();
  expect(await renderWorkspace(sb)).not.toContain("[JEV:");
  expect(await report(sb, sb.project)).not.toContain("Jev:");
});

test.concurrent("Jev readiness: broken wrapper symlink is unavailable", async () => {
  using sb = createSandbox();
  mkdirSync(join(cfgOf(sb), "jev"), { recursive: true });
  symlinkSync(sb.path("removed/jev.sh"), join(cfgOf(sb), "jev/jev.sh"));
  expect(await renderWorkspace(sb)).toEndWith(unavailable("missing executable wrapper"));
});

test.concurrent("Jev readiness: a non-executable wrapper or directory cannot be ready", async () => {
  using sb = createSandbox();
  const wrapper = join(cfgOf(sb), "jev/jev.sh");
  mkdirSync(join(cfgOf(sb), "jev"), { recursive: true });
  copyFileSync(JEV_BUNDLE, wrapper);
  chmodSync(wrapper, 0o644);
  expect(await renderWorkspace(sb)).toEndWith(unavailable("missing executable wrapper"));
  rmSync(wrapper);
  mkdirSync(wrapper);
  expect(await renderWorkspace(sb)).toEndWith(unavailable("missing executable wrapper"));
});

test.concurrent("Jev readiness: a self-looping wrapper symlink is unavailable", async () => {
  using sb = createSandbox();
  for (const root of [cfgOf(sb), sb.codexHome]) {
    mkdirSync(join(root, "jev"), { recursive: true });
    symlinkSync(join(root, "jev/jev.sh"), join(root, "jev/jev.sh"));
  }
  expect(await renderWorkspace(sb)).toEndWith(unavailable("missing executable wrapper"));
  expect(await report(sb, sb.project)).toContain("Jev: unavailable — missing executable wrapper\n");
});

test.concurrent("Jev readiness: a file where the jev directory belongs is unpublished", async () => {
  using sb = createSandbox();
  put(join(cfgOf(sb), "jev"), "");
  put(join(sb.codexHome, "jev"), "");
  expect(await renderWorkspace(sb)).not.toContain("[JEV:");
  expect(await report(sb, sb.project)).not.toContain("Jev:");
});

test.concurrent("Jev readiness: a workspace path under a regular file keeps readiness", async () => {
  using sb = createSandbox();
  await publishJev(cfgOf(sb), "claude");
  sb.write("file", "");
  expect(plain(await render(sb, { payload: payload(sb.path("file/x")) }))).toBe(
    "Opus | ctx:1k/200k | [JEV:READY]",
  );
});

test.concurrent("Jev readiness: the report names missing Bun using only local prerequisites", async () => {
  using sb = createSandbox();
  await publishJev(sb.codexHome, "codex");
  const bin = sb.path("bin");
  mkdirSync(bin);
  const found = Bun.which("git");
  if (found !== null) symlinkSync(found, join(bin, "git"));
  expect(await report(sb, sb.project, { PATH: bin })).toContain("Jev: unavailable — missing bun\n");
});

test.concurrent("Jev readiness: hosts never fall back to another profile's wrapper", async () => {
  using sb = createSandbox();
  await publishJev(cfgOf(sb), "claude");
  expect(await report(sb, sb.project)).not.toContain("Jev:");
  rmSync(join(cfgOf(sb), "jev/jev.sh"));
  await publishJev(sb.codexHome, "codex");
  expect(await renderWorkspace(sb)).not.toContain("[JEV:");
});

test.concurrent("Jev readiness: explicit config root takes priority on both hosts", async () => {
  using sb = createSandbox();
  const explicit = sb.path("explicit profile");
  await publishJev(explicit, "claude", { TOOLU_CONFIG_DIR: explicit });
  expect(await renderWorkspace(sb, { TOOLU_CONFIG_DIR: explicit })).toEndWith(READY);
  expect(await report(sb, sb.project, { TOOLU_CONFIG_DIR: explicit })).toContain("Jev: ready\n");
});

test.concurrent("Jev readiness: Claude still renders readiness when payload has no workspace", async () => {
  using sb = createSandbox();
  await publishJev(cfgOf(sb), "claude");
  expect(await render(sb, { payload: "{}" })).toBe(
    `\x1b[36mClaude\x1b[0m${SEP}\x1b[35mctx:0/0\x1b[0m${SEP}${READY}`,
  );
});

test.concurrent("Jev readiness: a payload without a cwd omits project state; the report defaults to its cwd", async () => {
  using sb = createSandbox();
  await publishJev(cfgOf(sb), "claude");
  repo(sb.project);
  sb.write(".claude/tmp/quality-gate-status.json", '{"status":"failing","reason":"fixture"}');
  expect(await render(sb, { payload: "{}", cwd: sb.project })).toBe(
    `\x1b[36mClaude\x1b[0m${SEP}\x1b[35mctx:0/0\x1b[0m${SEP}${READY}`,
  );

  sb.write(".codex/tmp/quality-gate-status.json", '{"status":"failing","reason":"fixture"}');
  const out = await report(sb, undefined, {}, sb.project);
  expect(out).toContain("Repository: ");
  expect(out).toContain("Quality gate: failing — fixture\n");
  expect(out).toContain("Working tree: staged 0, unstaged 0, untracked 2\n");
});

test.concurrent("Jev readiness: an apostrophe in the workspace is literal path data", async () => {
  using sb = createSandbox();
  await publishJev(cfgOf(sb), "claude");
  const dir = sb.path("user's project");
  put(join(dir, ".keep"), "");
  expect(await render(sb, { payload: payload(dir).replace('"Opus"', '"Claude"') })).toBe(
    `\x1b[36mClaude\x1b[0m${SEP}\x1b[35mctx:1k/200k\x1b[0m${SEP}\x1b[1muser's project\x1b[0m${SEP}${READY}`,
  );
});
