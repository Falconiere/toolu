/**
 * The core-dependency check of pr-babysit, python-quality, rust-quality and
 * ts-quality, each run through its own hooks.json launcher (#269, #265,
 * ported from dependency.bats). Each case states the expected output: the
 * warning, byte for byte as the bash `check-toolu.sh` printed it, or silence.
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
const PORTED = ["pr-babysit", "python-quality", "rust-quality", "ts-quality"];
const NO_CODEX = Bun.which("codex") === null;
const WARNING =
  "WARN: this plugin requires the toolu core. Install it first with: codex plugin add toolu@toolu";

/** The warning as bash's `jq -n` printed it. */
const WARNING_OUTPUT = `{
  "hookSpecificOutput": {
    "hookEventName": "SessionStart",
    "additionalContext": "${WARNING}"
  }
}
`;

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

/** Listing, `codex plugin list --json` stdout, its exit code, and whether toolu counts as missing. */
const LISTINGS: Array<[string, string, number, boolean]> = [
  ["installed and enabled", core({ installed: true, enabled: true }), 0, false],
  ["flags absent", core({}), 0, false],
  ["missing", '{"installed":[]}', 0, true],
  ["disabled", core({ installed: true, enabled: false }), 0, true],
  ["not installed", core({ installed: false, enabled: true }), 0, true],
  ["list fails", '{"installed":[]}', 1, false],
  ["malformed JSON", "not json", 0, true],
  ["unindexable entry", JSON.stringify({ installed: ["x", { pluginId: "toolu@toolu" }] }), 0, true],
];

/** Host, its environment, and whether the check runs there (native Codex only). */
const HOSTS: Array<[string, (plugin: string) => EnvPatch, boolean]> = [
  ["native Codex", (plugin) => ({ PLUGIN_ROOT: plugin }), true],
  ["Codex override without PLUGIN_ROOT", () => ({ TOOLU_HOST_OVERRIDE: "codex" }), false],
  ["Claude", () => ({}), false],
  [
    "Claude override with PLUGIN_ROOT",
    (plugin) => ({ PLUGIN_ROOT: plugin, TOOLU_HOST_OVERRIDE: "claude" }),
    false,
  ],
];

for (const [listing, stdout, code, missing] of LISTINGS) {
  for (const [host, hostEnv, checks] of HOSTS) {
    test.concurrent(`check-toolu: ${listing}, ${host}`, async () => {
      using sb = createSandbox();
      const path = `${codexBin(sb, stdout, code)}:${process.env["PATH"] ?? ""}`;
      const base = { HOME: sb.home, PATH: path };
      const expected = missing && checks ? WARNING_OUTPUT : "";
      for (const plugin of PORTED) {
        const root = join(PLUGINS, plugin);
        const env = { ...base, CLAUDE_PLUGIN_ROOT: root, ...hostEnv(root) };
        const res = await runStartupHook(root, "check-toolu", sb, env);
        expect(res.exitCode).toBe(0);
        expect(res.stdout).toBe(expected);
      }
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
