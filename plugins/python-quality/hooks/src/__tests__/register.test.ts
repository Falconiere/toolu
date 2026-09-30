/**
 * AC-3 and AC-4 (#266), ported from the register bats: the real SessionStart
 * `register` bundle, behind its hooks.json launcher, publishes exactly one
 * python-quality module into each host's registry, prunes only this plugin's
 * stale entries, is byte- and mtime-stable when re-run, and the published
 * module fires through the post-tools bundle only while the plugin is installed.
 */
import { expect, test } from "bun:test";
import { mkdirSync, readdirSync, readFileSync, statSync, utimesSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { launcherCommand } from "@toolu/core/launcher";
import { runPostBundle } from "@toolu/conformance/harness/posttool";
import { installPlugins, pretoolEnv } from "@toolu/conformance/harness/pretool";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch } from "@toolu/conformance/harness/spawn";
import { PY_PROJECT } from "./cases-types.ts";
import { PLUGIN_ROOT } from "./golden-harness.ts";

const MODULE = "python-quality@toolu__python-quality.js";
const BUNDLE = join(PLUGIN_ROOT, "hooks/dist/post-tool-use.js");

type Root = { name: string; env: (sb: Sandbox) => EnvPatch; dir: (sb: Sandbox) => string };

const ROOTS: Root[] = [
  { name: "Claude", env: () => ({}), dir: (sb) => join(sb.home, ".claude") },
  {
    name: "Codex",
    env: (sb) => ({ PLUGIN_ROOT: PLUGIN_ROOT, CODEX_HOME: sb.codexHome }),
    dir: (sb) => sb.codexHome,
  },
  {
    name: "TOOLU_CONFIG_DIR",
    env: (sb) => ({ TOOLU_CONFIG_DIR: join(sb.root, "cfg") }),
    dir: (sb) => join(sb.root, "cfg"),
  },
];

function register(sb: Sandbox, env: EnvPatch = {}) {
  const command = launcherCommand({
    plugin: "python-quality",
    event: "SessionStart",
    entry: "register",
  });
  return run(["/bin/sh", "-c", command], {
    cwd: sb.project,
    env: { HOME: sb.home, CLAUDE_PLUGIN_ROOT: PLUGIN_ROOT, TOOLU_BUN: process.execPath, ...env },
    stdin: "{}",
  });
}

function postDir(root: string): string {
  return join(root, "toolu", "post-tools.d");
}

for (const root of ROOTS) {
  test.concurrent(`publishes exactly the bundle under the ${root.name} root, silently`, async () => {
    using sb = createSandbox();
    const res = await register(sb, root.env(sb));
    expect(res).toMatchObject({ exitCode: 0, stdout: "" });
    const dir = postDir(root.dir(sb));
    expect(readdirSync(dir)).toEqual([MODULE]);
    expect(readFileSync(join(dir, MODULE))).toEqual(readFileSync(BUNDLE));
  });
}

test.concurrent("prunes only its own stale entries and aged tmp residue", async () => {
  using sb = createSandbox();
  const dir = postDir(join(sb.home, ".claude"));
  mkdirSync(dir, { recursive: true });
  const put = (name: string, ageMs = 0) => {
    writeFileSync(join(dir, name), "x\n");
    const when = new Date(Date.now() - ageMs);
    utimesSync(join(dir, name), when, when);
  };
  put("python-quality@toolu__python-quality.sh");
  put("python-quality@toolu__old-concern.sh");
  put("rust-quality@toolu__rust-quality.sh");
  put("python-quality@toolu__python-quality.js.tmp.111", 120_000);
  put("python-quality@toolu__python-quality.js.tmp.222", 5_000);
  put("rust-quality@toolu__rust-quality.sh.tmp.333", 120_000);
  expect((await register(sb)).exitCode).toBe(0);
  expect(readdirSync(dir).toSorted()).toEqual([
    MODULE,
    "python-quality@toolu__python-quality.js.tmp.222",
    "rust-quality@toolu__rust-quality.sh",
    "rust-quality@toolu__rust-quality.sh.tmp.333",
  ]);
});

test.concurrent("a second run leaves the module byte- and mtime-stable", async () => {
  using sb = createSandbox();
  const file = join(postDir(join(sb.home, ".claude")), MODULE);
  expect((await register(sb)).exitCode).toBe(0);
  const past = new Date(Date.now() - 60_000);
  utimesSync(file, past, past);
  const before = statSync(file).mtimeMs;
  expect(await register(sb)).toMatchObject({ exitCode: 0, stdout: "" });
  expect(statSync(file).mtimeMs).toBe(before);
  expect(readFileSync(file)).toEqual(readFileSync(BUNDLE));
});

const BAD = "x = 1  # type: ignore\n";

/** A blanket `# type: ignore` Write through the post-tools bundle, with python-quality registered for `host`. */
async function violation(sb: Sandbox, host: "claude" | "codex", installed: boolean) {
  for (const [rel, body] of Object.entries(PY_PROJECT)) sb.write(rel, body);
  sb.git("add", ...Object.keys(PY_PROJECT));
  sb.git("commit", "-q", "-m", "python project");
  const specs = installed ? ["toolu@toolu", "python-quality@toolu"] : ["toolu@toolu"];
  installPlugins(sb, ...specs);
  const codex = { PLUGIN_ROOT: PLUGIN_ROOT, CODEX_HOME: sb.codexHome };
  expect((await register(sb, host === "codex" ? codex : {})).exitCode).toBe(0);
  if (host === "codex") {
    const snapshot = { version: 1, status: "ready", plugins: specs };
    writeFileSync(
      join(sb.codexHome, "toolu", "codex-plugins.json"),
      `${JSON.stringify(snapshot)}\n`,
    );
  }
  sb.write("app.py", BAD);
  const payload = {
    session_id: "s",
    cwd: sb.project,
    hook_event_name: "PostToolUse",
    tool_name: "Write",
    tool_input: { file_path: sb.path("app.py"), content: BAD },
    tool_response: { success: true },
  };
  const call = { cwd: sb.project, env: pretoolEnv(sb, host), stdin: JSON.stringify(payload) };
  return runPostBundle(sb, call);
}

for (const host of ["claude", "codex"] as const) {
  test.concurrent(`the module fires while installed and is gated off when absent [${host}]`, async () => {
    using on = createSandbox({ git: true });
    using off = createSandbox({ git: true });
    const [fired, gated] = await Promise.all([
      violation(on, host, true),
      violation(off, host, false),
    ]);
    expect(fired.exitCode).toBe(0);
    expect(fired.stdout).toContain("Forbidden suppression");
    expect(gated).toMatchObject({ exitCode: 0, stdout: "" });
  });
}
