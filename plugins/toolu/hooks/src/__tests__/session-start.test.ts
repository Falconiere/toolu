/**
 * toolu's SessionStart bundle (#263, ported from session-start.bats): the
 * side effects bash had and the runtime line #250 added. Context parity with
 * the bash hook lives in lifecycle-golden.test.ts.
 */
import { expect, test } from "bun:test";
import {
  chmodSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { launcherHook, missingRuntimeMessage, runtimeDiagnostic } from "@toolu/core/launcher";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch } from "@toolu/conformance/harness/spawn";
import { hookCommand } from "@toolu/conformance/harness/startup";
import { z } from "zod";
import { PLUGIN, bundleArgv, launchCount, prepare, type LifecycleCase } from "./lifecycle-cases.ts";

const RUNTIME = runtimeDiagnostic(process.execPath, Bun.version).systemMessage;
const COMMAND = hookCommand(PLUGIN, "SessionStart", "session-start");
const OutputSchema = z.strictObject({
  hookSpecificOutput: z.strictObject({
    hookEventName: z.literal("SessionStart"),
    additionalContext: z.string(),
  }),
  systemMessage: z.string(),
});

const START: LifecycleCase = {
  name: "start",
  hook: "session-start",
  stdin: '{"source":"startup"}',
};

/** Run the bundle `times` times in one prepared sandbox; returns each stdout. */
async function runTimes(
  c: LifecycleCase,
  times: number,
  setup?: (sb: ReturnType<typeof prepare>) => void,
) {
  const prepared = prepare(c);
  setup?.(prepared);
  const outputs: string[] = [];
  try {
    for (let i = 0; i < times; i += 1) {
      const res = await run(bundleArgv("session-start"), {
        cwd: prepared.cwd,
        env: prepared.env,
        stdin: c.stdin,
      });
      expect(res.exitCode).toBe(0);
      outputs.push(res.stdout);
    }
    return { outputs, prepared };
  } catch (error) {
    prepared.sb[Symbol.dispose]();
    throw error;
  }
}

test("hooks.json runs one launcher entry on every SessionStart source", async () => {
  const hooks: unknown = await Bun.file(join(PLUGIN, "hooks", "hooks.json")).json();
  expect(hooks).toMatchObject({
    hooks: {
      SessionStart: expect.arrayContaining([
        {
          matcher: "startup|resume|clear|compact",
          hooks: [launcherHook({ plugin: "toolu", event: "SessionStart", entry: "session-start" })],
        },
      ]),
    },
  });
  expect(await launchCount("SessionStart", "session-start")).toBe(1);
});

test("the launcher reports the title and the Bun that runs it", async () => {
  using sb = createSandbox({ git: true });
  const env: EnvPatch = {
    PATH: "/usr/bin:/bin",
    HOME: sb.home,
    TOOLU_BUN: process.execPath,
    CLAUDE_PLUGIN_ROOT: PLUGIN,
  };
  const res = await run(["sh", "-c", COMMAND], {
    cwd: sb.project,
    env,
    stdin: '{"source":"resume"}',
  });
  expect(res).toMatchObject({ exitCode: 0 });
  const out = OutputSchema.parse(JSON.parse(res.stdout));
  expect(out.systemMessage).toBe(`Session resumed\n${RUNTIME}`);
  expect(out.hookSpecificOutput.additionalContext).toContain("Session Protocol — project");
});

test("clear and compact carry the title only", async () => {
  for (const source of ["clear", "compact"]) {
    const { outputs, prepared } = await runTimes(
      { ...START, stdin: JSON.stringify({ source }) },
      1,
    );
    prepared.sb[Symbol.dispose]();
    expect(OutputSchema.parse(JSON.parse(outputs[0] ?? "")).systemMessage).toBe(
      source === "clear" ? "Context cleared" : "Context compacted",
    );
  }
});

test("without Bun the session still starts with an advisory", async () => {
  using sb = createSandbox();
  const env: EnvPatch = { PATH: "/usr/bin:/bin", HOME: sb.home, CLAUDE_PLUGIN_ROOT: PLUGIN };
  const res = await run(["sh", "-c", COMMAND], { cwd: sb.project, env, stdin: "{}" });
  expect(res.exitCode).toBe(0);
  expect(JSON.parse(res.stdout)).toEqual({ systemMessage: missingRuntimeMessage("toolu") });
});

test("disabled context reports the runtime on startup only and writes no readiness marker", async () => {
  const off: LifecycleCase = { ...START, userConfig: { hooks: { "session-start": false } } };
  const { outputs, prepared } = await runTimes(off, 1);
  using _sb = prepared.sb;
  expect(JSON.parse(outputs[0] ?? "")).toEqual({ systemMessage: `Toolu is on!\n${RUNTIME}` });
  expect(existsSync(join(prepared.sb.home, ".claude", "toolu", ".session-start-ready"))).toBe(
    false,
  );
  const compact = await run(bundleArgv("session-start"), {
    cwd: prepared.cwd,
    env: prepared.env,
    stdin: '{"source":"compact"}',
  });
  expect(compact).toMatchObject({ exitCode: 0, stdout: "" });
});

test("OpenCode routes delegation by subagent_type, not a model argument", async () => {
  const opencode: LifecycleCase = {
    ...START,
    env: { TOOLU_HOST_OVERRIDE: "opencode", TOOLU_PROJECT_CONFIG_DIRNAME: ".opencode" },
  };
  const { outputs, prepared } = await runTimes(opencode, 1);
  using _sb = prepared.sb;
  const out = outputs[0] ?? "";
  expect(out).toContain("Route every `task` call by `subagent_type`");
  expect(out).toContain("`agent.<id>.model`");
  expect(out).not.toContain("Pass `model:`");
});

const CODEX_LIST =
  '{"installed":[{"pluginId":"toolu@toolu","name":"toolu","marketplaceName":"toolu","installed":true}],"available":[]}';

test("Codex snapshots plugins once and prunes modules of absent plugins", async () => {
  const codex: LifecycleCase = { ...START, host: "codex", codexList: CODEX_LIST };
  const { prepared } = await runTimes(codex, 1, ({ sb }) => {
    const log = join(sb.root, "codex-calls.log");
    writeFileSync(
      join(sb.root, "bin", "codex"),
      `#!/bin/sh\necho call >> '${log}'\nprintf '%s\\n' '${CODEX_LIST}'\n`,
    );
    const dir = join(sb.codexHome, "toolu", "pre-tools.d");
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, "gone@toolu__x.js"), "export default {};\n");
    writeFileSync(join(dir, "toolu@toolu__y.js"), "export default {};\n");
  });
  using sb = prepared.sb;
  const snapshot: unknown = JSON.parse(
    readFileSync(join(sb.codexHome, "toolu", "codex-plugins.json"), "utf8"),
  );
  expect(snapshot).toMatchObject({ status: "ready", plugins: ["toolu@toolu"] });
  expect(readFileSync(join(sb.root, "codex-calls.log"), "utf8")).toBe("call\n");
  expect(existsSync(join(sb.codexHome, "toolu", "pre-tools.d", "gone@toolu__x.js"))).toBe(false);
  expect(existsSync(join(sb.codexHome, "toolu", "pre-tools.d", "toolu@toolu__y.js"))).toBe(true);
});

test("Codex without its CLI prunes nothing; Claude writes no snapshot", async () => {
  const codex = await runTimes({ ...START, host: "codex" }, 1, ({ sb }) => {
    mkdirSync(join(sb.codexHome, "toolu", "pre-tools.d"), { recursive: true });
    writeFileSync(join(sb.codexHome, "toolu", "pre-tools.d", "gone@toolu__x.js"), "");
  });
  using codexSb = codex.prepared.sb;
  expect(existsSync(join(codexSb.codexHome, "toolu", "pre-tools.d", "gone@toolu__x.js"))).toBe(
    true,
  );
  const claude = await runTimes(START, 1);
  using claudeSb = claude.prepared.sb;
  expect(existsSync(join(claudeSb.home, ".claude", "toolu", "codex-plugins.json"))).toBe(false);
  expect(existsSync(join(claudeSb.codexHome, "toolu", "codex-plugins.json"))).toBe(false);
});

test("the legacy statusline symlink goes, a user's real file stays", async () => {
  const linked = await runTimes(START, 1, ({ sb }) => {
    mkdirSync(join(sb.home, ".claude", "toolu"), { recursive: true });
    symlinkSync(join(sb.home, "gone.sh"), join(sb.home, ".claude", "toolu", "statusline.sh"));
  });
  using a = linked.prepared.sb;
  expect(() => lstatSync(join(a.home, ".claude", "toolu", "statusline.sh"))).toThrow();
  const real = await runTimes(START, 1, ({ sb }) => {
    mkdirSync(join(sb.home, ".claude", "toolu"), { recursive: true });
    writeFileSync(join(sb.home, ".claude", "toolu", "statusline.sh"), "user-owned");
  });
  using b = real.prepared.sb;
  expect(readFileSync(join(b.home, ".claude", "toolu", "statusline.sh"), "utf8")).toBe(
    "user-owned",
  );
});

test("one-time notices and the permission write happen on the first session only", async () => {
  const { outputs, prepared } = await runTimes({ ...START, firstRun: true }, 2);
  using sb = prepared.sb;
  const [first = "", second = ""] = outputs.map(
    (out) => OutputSchema.parse(JSON.parse(out)).hookSpecificOutput.additionalContext,
  );
  for (const marker of [
    "no longer prompt",
    "workflow skills moved to delivery-flow",
    "one time only",
  ]) {
    expect(first).toContain(marker);
    expect(second).not.toContain(marker);
  }
  const settings: unknown = JSON.parse(
    readFileSync(join(prepared.cwd, ".claude", "settings.local.json"), "utf8"),
  );
  expect(settings).toMatchObject({ permissions: { allow: expect.arrayContaining(["Bash(*)"]) } });
  expect(existsSync(join(sb.home, ".claude", "toolu", ".gate-preset-notice-v6"))).toBe(true);
});

test("state of a deleted branch is swept, the live branch's is kept", async () => {
  const { prepared } = await runTimes({ ...START, branch: "feat/live" }, 1, ({ cwd }) => {
    const dir = join(cwd, ".claude", "tmp", "push-review");
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, "gone_branch.json"), '{"version":2}');
    writeFileSync(join(dir, "feat_live.json"), '{"version":2}');
  });
  using _sb = prepared.sb;
  const dir = join(prepared.cwd, ".claude", "tmp", "push-review");
  expect(existsSync(join(dir, "gone_branch.json"))).toBe(false);
  expect(existsSync(join(dir, "feat_live.json"))).toBe(true);
});

test.skipIf(process.getuid?.() === 0)(
  "an unreadable state dir does not stop the session",
  async () => {
    const dir = { path: "" };
    const { outputs, prepared } = await runTimes(START, 1, ({ cwd }) => {
      dir.path = join(cwd, ".claude", "tmp", "push-review");
      mkdirSync(dir.path, { recursive: true });
      writeFileSync(join(dir.path, "gone_branch.json"), '{"version":2}');
      chmodSync(dir.path, 0o000);
    });
    chmodSync(dir.path, 0o755);
    using _sb = prepared.sb;
    expect(OutputSchema.parse(JSON.parse(outputs[0] ?? "")).systemMessage).toStartWith(
      "Toolu is on!",
    );
  },
);

test.skipIf(process.getuid?.() === 0)(
  "an unremovable legacy symlink does not stop the session",
  async () => {
    const dir = { path: "" };
    const { outputs, prepared } = await runTimes(START, 1, ({ sb }) => {
      dir.path = join(sb.home, ".claude", "toolu");
      mkdirSync(dir.path, { recursive: true });
      symlinkSync(join(sb.home, "gone.sh"), join(dir.path, "statusline.sh"));
      chmodSync(dir.path, 0o555);
    });
    chmodSync(dir.path, 0o755);
    using _sb = prepared.sb;
    const out = OutputSchema.parse(JSON.parse(outputs[0] ?? ""));
    expect(out.hookSpecificOutput.additionalContext).toContain("Session Protocol — project");
  },
);
