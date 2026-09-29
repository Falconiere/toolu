/**
 * jev's SessionStart bundle through its real hooks.json launcher (#269,
 * ported from session-start.bats). The context strings are asserted whole:
 * they are the bash hook's text for the same environment.
 */
import { expect, test } from "bun:test";
import {
  chmodSync,
  cpSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readlinkSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { readHostOutcome } from "@toolu/conformance/harness/hosts";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch, type RunResult } from "@toolu/conformance/harness/spawn";
import {
  runStartupHook,
  startupEnv,
  startupRoot,
  type StartupHost,
} from "@toolu/conformance/harness/startup";
import { z } from "zod";

const PLUGIN = resolve(import.meta.dir, "../../..");
const WRAPPER = "skills/jev/scripts/jev.sh";
const KEY = "local-test-key";

const OutputSchema = z.strictObject({
  hookSpecificOutput: z.strictObject({
    hookEventName: z.literal("SessionStart"),
    additionalContext: z.string(),
  }),
});

/** The context, after checking the output against both hosts' SessionStart contract. */
function contextOf(res: RunResult): string {
  expect(res.exitCode).toBe(0);
  expect(res.stderr).toBe("");
  expect(res.stdout.endsWith("}\n")).toBe(true);
  const context = OutputSchema.parse(JSON.parse(res.stdout)).hookSpecificOutput.additionalContext;
  for (const host of ["claude", "codex"] as const) {
    expect(readHostOutcome(host, "SessionStart", res)).toEqual({ effect: "allow", context });
  }
  return context;
}

/** The bash hook's mandate text for `wrapper` and plugin root `plugin`. */
function mandate(wrapper: string, plugin: string): string {
  return `Jev is mandatory on every task containing semantic decisions. After initial exploration, identify useful judgments over supplied evidence; you MUST call "${wrapper}" before the decision it informs. Reassess after new evidence, failed hypotheses, or changed requirements. Batch independent questions in one ask call. Reuse unchanged evidence and questions rather than repeating calls. If a task has no semantic decision, say so in one sentence rather than skipping silently. Syntax and linked examples: ${plugin}/skills/jev/SKILL.md. Keep exact rules, tests, and code verification deterministic. On service failure, state the limitation and use an explicit evidence fallback. Jev never replaces tests or authorization.`;
}

/** The bash hook's fallback text for the space-prefixed `missing` list. */
function fallback(missing: string): string {
  return `Jev unavailable (missing:${missing}). Set TYPESAFE_API_KEY in the agent's launch environment and install curl/jq. Jev is mandatory on every task once available; until then, state the limitation once per task and use an explicit reasoning/evidence fallback; never invent a Jev result. Do not read credentials from .env.`;
}

function hook(sb: Sandbox, host: StartupHost, env: EnvPatch = {}, plugin = PLUGIN) {
  const base = { ...startupEnv(host, sb, plugin), TYPESAFE_API_KEY: KEY };
  return runStartupHook(plugin, "session-start", sb, { ...base, ...env });
}

/** A copy of the plugin (bundle and skill) at `dir`. */
function install(dir: string): string {
  mkdirSync(dir, { recursive: true });
  cpSync(join(PLUGIN, "hooks"), join(dir, "hooks"), { recursive: true });
  cpSync(join(PLUGIN, "skills"), join(dir, "skills"), { recursive: true });
  return dir;
}

test.concurrent("publishes jev.sh and states the mandate on Claude", async () => {
  using sb = createSandbox();
  const context = contextOf(await hook(sb, "claude"));
  const dst = join(sb.home, ".claude/jev/jev.sh");
  expect(readlinkSync(dst)).toBe(join(PLUGIN, WRAPPER));
  expect(context).toBe(mandate(dst, PLUGIN));
  expect(context).not.toContain(KEY);
});

test.concurrent("publishes under CODEX_HOME with spaces on the native Codex host", async () => {
  using sb = createSandbox();
  const codexHome = join(sb.home, "codex profile");
  const context = contextOf(await hook(sb, "codex", { CODEX_HOME: codexHome }));
  expect(readlinkSync(join(codexHome, "jev/jev.sh"))).toBe(join(PLUGIN, WRAPPER));
  expect(context).toBe(mandate(join(codexHome, "jev/jev.sh"), PLUGIN));
  expect(existsSync(join(sb.home, ".claude"))).toBe(false);
});

test.concurrent("TOOLU_CONFIG_DIR takes precedence over the Codex root", async () => {
  using sb = createSandbox();
  const custom = join(sb.root, "custom profile");
  contextOf(await hook(sb, "codex", { TOOLU_CONFIG_DIR: custom }));
  expect(readlinkSync(join(custom, "jev/jev.sh"))).toBe(join(PLUGIN, WRAPPER));
  expect(existsSync(join(startupRoot("codex", sb), "jev"))).toBe(false);
});

test.concurrent("without the key: the actionable fallback, never a MUST call", async () => {
  using sb = createSandbox();
  const context = contextOf(await hook(sb, "claude", { TYPESAFE_API_KEY: undefined }));
  expect(context).toBe(fallback(" TYPESAFE_API_KEY"));
});

test.concurrent("missing tools are listed in the bash order", async () => {
  using sb = createSandbox();
  const bundle = join(PLUGIN, "hooks/dist/session-start.js");
  const empty = mkdtempSync(join(sb.root, "empty-path-"));
  const env = { ...startupEnv("claude", sb, PLUGIN), PATH: empty, TYPESAFE_API_KEY: undefined };
  const res = await run([process.execPath, bundle], { cwd: sb.project, env, stdin: "{}" });
  expect(contextOf(res)).toBe(fallback(" jq curl TYPESAFE_API_KEY"));
});

test.concurrent("paths with quotes, spaces and newlines stay JSON string data", async () => {
  using sb = createSandbox();
  const plugin = install(join(sb.root, 'plugin "cache"\nfolder'));
  const config = join(sb.root, 'profile "quoted"\nfolder');
  const context = contextOf(await hook(sb, "claude", { TOOLU_CONFIG_DIR: config }, plugin));
  expect(context).toBe(mandate(join(config, "jev/jev.sh"), plugin));
  expect(readlinkSync(join(config, "jev/jev.sh"))).toBe(join(plugin, WRAPPER));
});

test.concurrent("refreshes a stale link and is idempotent", async () => {
  using sb = createSandbox();
  const dir = join(sb.home, ".claude/jev");
  mkdirSync(dir, { recursive: true });
  symlinkSync("/nonexistent/old/jev.sh", join(dir, "jev.sh"));
  contextOf(await hook(sb, "claude"));
  expect(readlinkSync(join(dir, "jev.sh"))).toBe(join(PLUGIN, WRAPPER));
  const inode = lstatSync(join(dir, "jev.sh")).ino;
  contextOf(await hook(sb, "claude"));
  expect(lstatSync(join(dir, "jev.sh")).ino).toBe(inode);
});

test.concurrent("a user's own file is kept and still named in the context", async () => {
  using sb = createSandbox();
  const dst = join(sb.home, ".claude/jev/jev.sh");
  mkdirSync(join(sb.home, ".claude/jev"), { recursive: true });
  writeFileSync(dst, "#!/usr/bin/env bash\necho user-override\n");
  chmodSync(dst, 0o755);
  const context = contextOf(await hook(sb, "claude"));
  expect(lstatSync(dst).isSymbolicLink()).toBe(false);
  expect(readFileSync(dst, "utf8")).toBe("#!/usr/bin/env bash\necho user-override\n");
  expect(context).toBe(mandate(dst, PLUGIN));
});

test.concurrent("a missing wrapper source is silent and publishes nothing", async () => {
  using sb = createSandbox();
  const plugin = join(sb.root, "fake-plugin");
  cpSync(join(PLUGIN, "hooks"), join(plugin, "hooks"), { recursive: true });
  const res = await hook(sb, "claude", {}, plugin);
  expect(res).toMatchObject({ exitCode: 0, stdout: "", stderr: "" });
  expect(existsSync(join(sb.home, ".claude/jev"))).toBe(false);
});

test.concurrent("an uncreatable config dir reports once and emits no context", async () => {
  using sb = createSandbox();
  const blocker = join(sb.root, "blocker");
  writeFileSync(blocker, "");
  const res = await hook(sb, "claude", { TOOLU_CONFIG_DIR: blocker });
  expect(res).toMatchObject({
    exitCode: 0,
    stdout: "",
    stderr: `jev: cannot create ${join(blocker, "jev")} — wrapper not published\n`,
  });
});

test.concurrent("a config dir that refuses the link reports once and emits no context", async () => {
  using sb = createSandbox();
  const dir = join(sb.home, ".claude/jev");
  mkdirSync(dir, { recursive: true });
  chmodSync(dir, 0o555);
  try {
    const res = await hook(sb, "claude");
    expect(res).toMatchObject({
      exitCode: 0,
      stdout: "",
      stderr: `jev: cannot publish ${join(dir, "jev.sh")}\n`,
    });
  } finally {
    chmodSync(dir, 0o755);
  }
});
