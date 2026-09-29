/**
 * The core-dependency check of pr-babysit, python-quality and rust-quality,
 * each run through its own hooks.json launcher (#269, ported from
 * dependency.bats). ts-quality still ships the bash `check-toolu.sh` (#265),
 * so every case is judged against it byte for byte, on the same `codex`.
 */
import { expect, test } from "bun:test";
import { chmodSync, mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { readHostOutcome } from "@toolu/conformance/harness/hosts";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { childEnv, run, type EnvPatch } from "@toolu/conformance/harness/spawn";
import { hookCommand, runStartupHook } from "@toolu/conformance/harness/startup";

const PLUGINS = resolve(import.meta.dir, "../../../..");
const REPO = resolve(PLUGINS, "..");
const BASH_REFERENCE = join(PLUGINS, "ts-quality/hooks/check-toolu.sh");
const PORTED = ["pr-babysit", "python-quality", "rust-quality"];
const NO_CODEX = Bun.which("codex") === null;
const WARNING =
  "WARN: this plugin requires the toolu core. Install it first with: codex plugin add toolu@toolu";

/** A bin dir whose `codex` prints `stdout` and exits `code`. */
function codexBin(sb: Sandbox, stdout: string, code = 0): string {
  const bin = join(sb.root, "bin");
  mkdirSync(bin, { recursive: true });
  writeFileSync(join(bin, "codex"), `#!/bin/sh\ncat <<'JSON'\n${stdout}\nJSON\nexit ${code}\n`);
  chmodSync(join(bin, "codex"), 0o755);
  return bin;
}

const core = (flags: object): string =>
  JSON.stringify({ installed: [{ pluginId: "toolu@toolu", ...flags }] });

const LISTINGS: Array<[string, string, number]> = [
  ["installed and enabled", core({ installed: true, enabled: true }), 0],
  ["flags absent", core({}), 0],
  ["missing", '{"installed":[]}', 0],
  ["disabled", core({ installed: true, enabled: false }), 0],
  ["not installed", core({ installed: false, enabled: true }), 0],
  ["list fails", '{"installed":[]}', 1],
  ["malformed JSON", "not json", 0],
  ["unindexable entry", JSON.stringify({ installed: ["x", { pluginId: "toolu@toolu" }] }), 0],
];

const HOSTS: Array<[string, (plugin: string) => EnvPatch]> = [
  ["native Codex", (plugin) => ({ PLUGIN_ROOT: plugin })],
  ["Codex override without PLUGIN_ROOT", () => ({ TOOLU_HOST_OVERRIDE: "codex" })],
  ["Claude", () => ({})],
  [
    "Claude override with PLUGIN_ROOT",
    (plugin) => ({ PLUGIN_ROOT: plugin, TOOLU_HOST_OVERRIDE: "claude" }),
  ],
];

for (const [listing, stdout, code] of LISTINGS) {
  for (const [host, hostEnv] of HOSTS) {
    test.concurrent(`matches bash check-toolu.sh: ${listing}, ${host}`, async () => {
      using sb = createSandbox();
      const path = `${codexBin(sb, stdout, code)}:${process.env["PATH"] ?? ""}`;
      const base = { HOME: sb.home, PATH: path };
      const reference = await run(["bash", BASH_REFERENCE], {
        cwd: sb.project,
        env: { ...base, ...hostEnv(join(PLUGINS, "ts-quality")) },
      });
      expect(reference.exitCode).toBe(0);
      for (const plugin of PORTED) {
        const root = join(PLUGINS, plugin);
        const env = { ...base, CLAUDE_PLUGIN_ROOT: root, ...hostEnv(root) };
        const res = await runStartupHook(root, "check-toolu", sb, env);
        expect(res.exitCode).toBe(0);
        expect(res.stdout).toBe(reference.stdout);
      }
      if (reference.stdout !== "") expect(reference.stdout).toContain(WARNING);
    });
  }
}

test.concurrent("the check never waits for SessionStart stdin EOF", async () => {
  using sb = createSandbox();
  const root = join(PLUGINS, "pr-babysit");
  const bin = codexBin(sb, '{"installed":[]}');
  const env = childEnv({
    HOME: sb.home,
    PATH: `${bin}:${process.env["PATH"] ?? ""}`,
    CLAUDE_PLUGIN_ROOT: root,
    PLUGIN_ROOT: root,
  });
  const proc = Bun.spawn(["sh", "-c", hookCommand(root, "SessionStart", "check-toolu")], {
    cwd: sb.project,
    env,
    stdin: "pipe",
    stdout: "pipe",
  });
  const outcome = await Promise.race([proc.exited, Bun.sleep(10_000).then(() => "timeout")]);
  if (outcome === "timeout") proc.kill("SIGKILL");
  expect(outcome).toBe(0);
  expect(await new Response(proc.stdout).text()).toContain(WARNING);
  await proc.stdin.end();
});

test.concurrent.skipIf(NO_CODEX)(
  "the real codex CLI: warns before toolu is added, silent after",
  async () => {
    using sb = createSandbox();
    const env = { HOME: sb.home, CODEX_HOME: sb.codexHome };
    const codex = (...args: string[]) =>
      run(["codex", "plugin", ...args, "--json"], { env, timeoutMs: 60_000 });
    expect((await codex("marketplace", "add", REPO)).exitCode).toBe(0);
    expect((await codex("add", "pr-babysit@toolu")).exitCode).toBe(0);
    const root = join(PLUGINS, "pr-babysit");
    const hookEnv = { ...env, CLAUDE_PLUGIN_ROOT: root, PLUGIN_ROOT: root };
    const before = await runStartupHook(root, "check-toolu", sb, hookEnv);
    expect(readHostOutcome("codex", "SessionStart", before)).toEqual({
      effect: "allow",
      context: WARNING,
    });
    expect((await codex("add", "toolu@toolu")).exitCode).toBe(0);
    const after = await runStartupHook(root, "check-toolu", sb, hookEnv);
    expect(after).toMatchObject({ exitCode: 0, stdout: "" });
  },
);
