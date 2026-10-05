import { expect, test } from "bun:test";
import { join, resolve } from "node:path";
import { z } from "zod";
import { bundlePath } from "../entry-command.ts";
import { createSandbox } from "../sandbox.ts";
import { childEnv, run, runHook } from "../spawn.ts";

const ROOT = resolve(import.meta.dir, "../../../../..");
const PLUGIN_ROOT = join(ROOT, "plugins/toolu");
const SAMPLE = bundlePath(PLUGIN_ROOT, "sample");
const EnvSchema = z.record(z.string(), z.string());

/** A real hook script that echoes the env it was spawned with. */
const ENV_HOOK = "process.stdout.write(JSON.stringify(process.env));\n";

test.concurrent("runHook spawns the committed sample bundle with bun", async () => {
  using sb = createSandbox({ git: true });
  const res = await runHook({
    host: "claude",
    sandbox: sb,
    pluginRoot: PLUGIN_ROOT,
    bundle: SAMPLE,
    stdin: {},
  });
  expect(res.exitCode).toBe(0);
  expect(res.timedOut).toBe(false);
  expect(JSON.parse(res.stdout)).toEqual({ kind: "allow" });
  expect(res.durationMs).toBeGreaterThan(0);
});

test.concurrent("claude hooks see CLAUDE_* roots, the override, and the sandbox HOME", async () => {
  using sb = createSandbox();
  const hook = sb.write("env-hook.ts", ENV_HOOK);
  const res = await runHook({
    host: "claude",
    sandbox: sb,
    pluginRoot: PLUGIN_ROOT,
    bundle: hook,
    stdin: "",
  });
  const env = EnvSchema.parse(JSON.parse(res.stdout));
  expect(env).toMatchObject({
    CLAUDE_PLUGIN_ROOT: PLUGIN_ROOT,
    CLAUDE_PROJECT_DIR: sb.project,
    CLAUDE_CONFIG_DIR: join(sb.home, ".claude"),
    TOOLU_HOST_OVERRIDE: "claude",
    HOME: sb.home,
  });
  expect(env.PLUGIN_ROOT).toBeUndefined();
});

test.concurrent("codex hooks see PLUGIN_ROOT and CODEX_HOME, and no parent CLAUDE_* leaks", async () => {
  using sb = createSandbox();
  const hook = sb.write("env-hook.ts", ENV_HOOK);
  // The developer's own agent session exports these; none may reach a hook under test.
  const parent = {
    PATH: "/bin",
    CLAUDE_PLUGIN_ROOT: "/leaked",
    CODEX_HOME: "/leaked",
    PLUGIN_ROOT: "/leaked",
    TOOLU_HOST_OVERRIDE: "claude",
  };
  expect(childEnv({}, parent)).toEqual({ PATH: "/bin" });
  const res = await runHook({
    host: "codex",
    sandbox: sb,
    pluginRoot: PLUGIN_ROOT,
    bundle: hook,
    stdin: "",
  });
  const env = EnvSchema.parse(JSON.parse(res.stdout));
  expect(env).toMatchObject({
    PLUGIN_ROOT: PLUGIN_ROOT,
    CODEX_HOME: sb.codexHome,
    TOOLU_HOST_OVERRIDE: "codex",
  });
  expect(Object.keys(env).filter((key) => key.startsWith("CLAUDE_"))).toEqual([]);
});

test.concurrent("an undefined env patch value unsets the key", async () => {
  const res = await run(["sh", "-c", 'printf "%s" "${HARNESS_PROBE-unset}"'], {
    env: { HARNESS_PROBE: undefined },
  });
  expect(res.stdout).toBe("unset");
  const set = await run(["sh", "-c", 'printf "%s" "$HARNESS_PROBE"'], {
    env: { HARNESS_PROBE: "on" },
  });
  expect(set.stdout).toBe("on");
});

test.concurrent("stdin is delivered and closed, and the exit code and stderr are captured", async () => {
  const res = await run(["sh", "-c", "cat; echo oops >&2; exit 3"], { stdin: "payload" });
  expect(res).toMatchObject({ exitCode: 3, stdout: "payload", stderr: "oops\n", timedOut: false });
});

test.concurrent("a timeout kills the whole process group and resolves quickly", async () => {
  const res = await run(["sh", "-c", "sleep 5; echo late"], { timeoutMs: 200 });
  expect(res.timedOut).toBe(true);
  expect(res.stdout).toBe("");
  expect(res.exitCode).toBe(137);
  expect(res.durationMs).toBeLessThan(4000);
});

test.concurrent("a missing binary resolves as exit 127 instead of throwing", async () => {
  const res = await run(["/nonexistent/toolu-harness-bin"]);
  expect(res).toMatchObject({
    exitCode: 127,
    stderr: "/nonexistent/toolu-harness-bin: command not found",
  });
});

test.concurrent("runHook rejects an ambiguous or missing target before spawning", () => {
  using sb = createSandbox();
  const base = { host: "claude" as const, sandbox: sb, pluginRoot: PLUGIN_ROOT, stdin: {} };
  expect(() => runHook({ ...base, bundle: SAMPLE, argv: ["true"] })).toThrow("not both");
  expect(() => runHook(base)).toThrow("pass bundle or argv");
  expect(() => runHook({ ...base, bundle: join(sb.root, "missing.js") })).toThrow(
    "bundle not found",
  );
});

test.concurrent("runHook runs explicit argv hooks with cwd at the sandbox project", async () => {
  using sb = createSandbox();
  const res = await runHook({
    host: "codex",
    sandbox: sb,
    pluginRoot: PLUGIN_ROOT,
    argv: ["pwd"],
    stdin: {},
  });
  expect(res.stdout.trim()).toBe(sb.project);
});

test.concurrent("a child that exits without reading a large stdin still resolves", async () => {
  const res = await run(["sh", "-c", "exit 4"], { stdin: "x".repeat(4_000_000) });
  expect(res).toMatchObject({ exitCode: 4, timedOut: false });
});

test.concurrent("a missing cwd is reported, not mistaken for a missing binary", async () => {
  const err = await run(["true"], { cwd: "/nonexistent/toolu-harness-cwd" }).then(
    () => null,
    (e: unknown) => e,
  );
  expect(String(err)).toContain("cwd does not exist: /nonexistent/toolu-harness-cwd");
});
