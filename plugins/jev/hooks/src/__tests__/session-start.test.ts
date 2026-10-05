/**
 * jev's SessionStart bundle through its real hooks.json launcher (#269,
 * ported from session-start.bats), including command execution without Bun on PATH.
 */
import { expect, test } from "bun:test";
import {
  chmodSync,
  cpSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readlinkSync,
  symlinkSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { bundlePath, entryArgv } from "@toolu/conformance/harness/entry-command";
import { readHostOutcome } from "@toolu/conformance/harness/hosts";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch, type RunResult } from "@toolu/conformance/harness/spawn";
import {
  runStartupHook,
  hookCommand,
  startupEnv,
  startupRoot,
  type StartupHost,
} from "@toolu/conformance/harness/startup";
import { z } from "zod";
import { startHttpsFixture } from "@toolu/conformance/https-fixture";

const runtimeCommand = (wrapper: string) =>
  `'${process.execPath.replaceAll("'", "'\\''")}' '${wrapper.replaceAll("'", "'\\''")}'`;

const PLUGIN = resolve(import.meta.dir, "../../..");
const WRAPPER = bundlePath("", "jev");
const KEY = "local-test-key";
const NODE = Bun.which("node");

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

/** The expected mandate for `wrapper` and plugin root `plugin`. */
function mandate(wrapper: string, plugin: string, command = runtimeCommand(wrapper)): string {
  return `Jev is mandatory on every task containing semantic decisions. After initial exploration, identify useful judgments over supplied evidence; you MUST call ${command} before the decision it informs. Published bundles use the hook's resolved Bun executable and do not require bun on PATH. Reassess after new evidence, failed hypotheses, or changed requirements. Batch independent questions in one ask call. Reuse unchanged evidence and questions rather than repeating calls. If a task has no semantic decision, say so in one sentence rather than skipping silently. Syntax and linked examples: ${plugin}/skills/jev/SKILL.md. Keep exact rules, tests, and code verification deterministic. On service failure, state the limitation and use an explicit evidence fallback. Jev never replaces tests or authorization.`;
}

function expectCredentialCheck(context: string): void {
  expect(context).toContain("The Jev hook did not receive TYPESAFE_API_KEY");
  expect(context).toContain(
    "check whether TYPESAFE_API_KEY is set in the command environment without printing its value",
  );
  expect(context).not.toContain("Jev unavailable (missing:");
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

test.concurrent("without the hook key: verify the command environment before declaring unavailable", async () => {
  using sb = createSandbox();
  const context = contextOf(await hook(sb, "claude", { TYPESAFE_API_KEY: undefined }));
  expectCredentialCheck(context);
});

test.concurrent("missing key is reported even without jq and curl", async () => {
  using sb = createSandbox();
  const env = { ...startupEnv("claude", sb, PLUGIN), TYPESAFE_API_KEY: undefined };
  const res = await run(entryArgv("jev", "session-start", PLUGIN), {
    cwd: sb.project,
    env,
    stdin: "{}",
  });
  expectCredentialCheck(contextOf(res));
});

test.concurrent("Bun off PATH and hook key absent still allow Jev in the command environment on both hosts", async () => {
  using sb = createSandbox();
  const fixture = await startHttpsFixture(["api.typesafe.ai"]);
  const empty = join(sb.root, "empty-bin");
  mkdirSync(empty);
  try {
    for (const host of ["claude", "codex"] as const) {
      const env = {
        ...startupEnv(host, sb, PLUGIN),
        PATH: empty,
        TOOLU_BUN: process.execPath,
        TYPESAFE_API_KEY: undefined,
        TOOLU_CONFIG_DIR: join(sb.root, `${host} 'quoted' $(no-command) profile`),
      };
      // Absolute sh starts the real launcher even with an empty PATH.
      const res = await run(
        ["/bin/sh", "-c", hookCommand(PLUGIN, "SessionStart", "session-start")],
        {
          cwd: sb.project,
          env,
          stdin: "{}",
        },
      );
      const wrapper = join(env.TOOLU_CONFIG_DIR, "jev/jev.sh");
      const context = contextOf(res);
      expect(context).toContain(runtimeCommand(wrapper));
      expectCredentialCheck(context);
      const command = context.split("you MUST call ")[1]?.split(" before the decision")[0];
      expect(command).toBe(runtimeCommand(wrapper));
      fixture.plan([
        {
          body: JSON.stringify({
            model: "jev-1.13.0",
            usage: { input_tokens: 3, output_tokens: 2 },
            answers: { q: { type: "noul", noul: 0.92 } },
          }),
        },
      ]);
      const probe = await run(["/bin/sh", "-c", `${command} noul probe -s evidence`], {
        cwd: sb.project,
        env: { ...env, ...fixture.env, TYPESAFE_API_KEY: KEY },
      });
      expect(probe.exitCode).toBe(0);
      expect(JSON.parse(probe.stdout)).toEqual({ q: { type: "noul", noul: 0.92 } });
      expect(probe.stderr).toBe("");
      expect(context).not.toContain(KEY);
      expect(fixture.requests[0]?.headers.authorization).toBe(`Bearer ${KEY}`);
    }
  } finally {
    await fixture.stop();
  }
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

test.concurrent("a user's executable shell override keeps its own interpreter", async () => {
  using sb = createSandbox();
  const dst = join(sb.home, ".claude/jev/jev.sh");
  mkdirSync(join(sb.home, ".claude/jev"), { recursive: true });
  writeFileSync(dst, "#!/usr/bin/env bash\necho user-override\n");
  chmodSync(dst, 0o755);
  const context = contextOf(await hook(sb, "claude"));
  expect(lstatSync(dst).isSymbolicLink()).toBe(false);
  expect(readFileSync(dst, "utf8")).toBe("#!/usr/bin/env bash\necho user-override\n");
  expect(context).toBe(mandate(dst, PLUGIN, `'${dst}'`));
  const command = context.split("you MUST call ")[1]?.split(" before the decision")[0];
  const probe = await run(["/bin/sh", "-c", command ?? ""], { cwd: sb.project });
  expect(probe.exitCode).toBe(0);
  expect(probe.stdout).toBe("user-override\n");
});

test.concurrent.skipIf(NODE === null)(
  "a user's executable JavaScript override keeps its interpreter without Bun on PATH",
  async () => {
    using sb = createSandbox();
    const dst = join(sb.home, ".claude/jev/jev.sh");
    mkdirSync(join(sb.home, ".claude/jev"), { recursive: true });
    const source = `#!${NODE}\nconsole.log("javascript-override");\n`;
    writeFileSync(dst, source, { mode: 0o755 });
    const env = { PATH: "/nonexistent", TYPESAFE_API_KEY: KEY };
    const res = await run(entryArgv("jev", "session-start", PLUGIN), {
      cwd: sb.project,
      env: { ...startupEnv("claude", sb, PLUGIN), ...env },
      stdin: "{}",
    });
    const context = contextOf(res);
    expect(lstatSync(dst).isSymbolicLink()).toBe(false);
    expect(readFileSync(dst, "utf8")).toBe(source);
    expect(context).toBe(mandate(dst, PLUGIN, `'${dst}'`));
    const command = context.split("you MUST call ")[1]?.split(" before the decision")[0];
    expect(command).toBe(`'${dst}'`);
    const probe = await run(["/bin/sh", "-c", command ?? ""], { cwd: sb.project, env });
    expect(probe.exitCode).toBe(0);
    expect(probe.stdout).toBe("javascript-override\n");
    expect(probe.stderr).toBe("");
  },
);

test.concurrent("a missing wrapper source is silent and publishes nothing", async () => {
  using sb = createSandbox();
  const plugin = join(sb.root, "fake-plugin");
  cpSync(join(PLUGIN, "hooks"), join(plugin, "hooks"), { recursive: true });
  unlinkSync(join(plugin, WRAPPER));
  const res = await hook(sb, "claude", {}, plugin);
  expect(res).toMatchObject({ exitCode: 0, stdout: "", stderr: "" });
  expect(existsSync(join(sb.home, ".claude/jev"))).toBe(false);
});

test.concurrent("a non-executable user wrapper reports the installation problem", async () => {
  using sb = createSandbox();
  const dst = join(sb.home, ".claude/jev/jev.sh");
  mkdirSync(join(sb.home, ".claude/jev"), { recursive: true });
  writeFileSync(dst, "user wrapper\n", { mode: 0o644 });
  const context = contextOf(await hook(sb, "claude"));
  expect(context).toBe(
    "Jev unavailable: published wrapper is not executable. Repair the Jev plugin installation. Until then, state the limitation once per task and use an explicit reasoning/evidence fallback; never invent a Jev result. Do not read credentials from .env.",
  );
  expect(readFileSync(dst, "utf8")).toBe("user wrapper\n");
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
