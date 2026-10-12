/**
 * #265, #426: the native SessionStart `register` publishes ts-quality's
 * manifest and bridge module into each host's registry, removes the Bun module
 * it replaces and only this plugin's stale entries, is byte- and mtime-stable
 * when re-run, and the rule fires through both post-tools hooks only while the
 * plugin is installed.
 */
import { expect, test } from "bun:test";
import { mkdirSync, readdirSync, readFileSync, statSync, utimesSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { installPlugins, nativeRegisterArgv } from "@toolu/conformance/harness/pretool";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch } from "@toolu/conformance/harness/spawn";
import { TS_PROJECT, type TsCase } from "../cases-types.ts";
import { PLUGIN_ROOT, dispatchStep, type Leg } from "../golden-harness.ts";

const MANIFEST = "ts-quality@toolu__ts-quality.json";
const BRIDGE = "ts-quality@toolu__ts-quality.sh";
const BUN_MODULE = "ts-quality@toolu__ts-quality.js";
const MANIFEST_BODY =
  '{"version":1,"spec":"ts-quality@toolu","name":"ts-quality","event":"tool/post","matcher":"*"}\n';

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
  return run(nativeRegisterArgv("ts-quality"), {
    cwd: sb.project,
    env: { HOME: sb.home, CLAUDE_PLUGIN_ROOT: PLUGIN_ROOT, ...env },
    stdin: "{}",
  });
}

function postDir(root: string): string {
  return join(root, "toolu", "post-tools.d");
}

for (const root of ROOTS) {
  test.concurrent(`publishes the manifest and bridge under the ${root.name} root`, async () => {
    using sb = createSandbox();
    const res = await register(sb, root.env(sb));
    expect(res).toMatchObject({ exitCode: 0, stdout: "", stderr: "" });
    const dir = postDir(root.dir(sb));
    expect(readdirSync(dir).toSorted()).toEqual([MANIFEST, BRIDGE]);
    expect(readFileSync(join(dir, MANIFEST), "utf8")).toBe(MANIFEST_BODY);
    expect(readFileSync(join(dir, BRIDGE), "utf8")).toContain(
      'exec "$bin" ts-quality hook post-tools --event PostToolUse',
    );
  });
}

test.concurrent("replaces the Bun module and prunes only its own stale entries", async () => {
  using sb = createSandbox();
  const dir = postDir(join(sb.home, ".claude"));
  mkdirSync(dir, { recursive: true });
  const put = (name: string, ageMs = 0) => {
    writeFileSync(join(dir, name), "x\n");
    const when = new Date(Date.now() - ageMs);
    utimesSync(join(dir, name), when, when);
  };
  put(BUN_MODULE);
  put(BRIDGE);
  put("ts-quality@toolu__old-concern.sh");
  put("rust-quality@toolu__rust-quality.sh");
  put("ts-quality@toolu__ts-quality.js.tmp.111", 120_000);
  put("ts-quality@toolu__ts-quality.js.tmp.222", 5_000);
  put("rust-quality@toolu__rust-quality.sh.tmp.333", 120_000);
  const res = await register(sb);
  expect(res.exitCode).toBe(0);
  expect(JSON.parse(res.stdout)).toEqual({
    systemMessage: "ts-quality's Bun module was replaced by its native rule in toolu.",
  });
  expect(readdirSync(dir).toSorted()).toEqual([
    "rust-quality@toolu__rust-quality.sh",
    "rust-quality@toolu__rust-quality.sh.tmp.333",
    "ts-quality@toolu__ts-quality.js.tmp.222",
    MANIFEST,
    BRIDGE,
  ]);
  expect(readFileSync(join(dir, BRIDGE), "utf8")).not.toBe("x\n");
});

test.concurrent("a second run leaves both files byte- and mtime-stable, silently", async () => {
  using sb = createSandbox();
  const dir = postDir(join(sb.home, ".claude"));
  expect((await register(sb)).exitCode).toBe(0);
  const past = new Date(Date.now() - 60_000);
  const files = [MANIFEST, BRIDGE].map((name) => join(dir, name));
  for (const file of files) utimesSync(file, past, past);
  const before = files.map((file) => [statSync(file).mtimeMs, readFileSync(file, "utf8")]);
  expect(await register(sb)).toMatchObject({ exitCode: 0, stdout: "", stderr: "" });
  expect(files.map((file) => [statSync(file).mtimeMs, readFileSync(file, "utf8")])).toEqual(before);
});

/** A `console.log` Write through `leg`'s post-tools hook, with ts-quality registered for `host`. */
async function violation(sb: Sandbox, host: "claude" | "codex", installed: boolean, leg: Leg) {
  for (const [rel, body] of Object.entries(TS_PROJECT)) sb.write(rel, body);
  sb.git("add", ...Object.keys(TS_PROJECT));
  sb.git("commit", "-q", "-m", "ts project");
  const specs = installed ? ["toolu@toolu", "ts-quality@toolu"] : ["toolu@toolu"];
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
  const c: TsCase = { name: "violation", project: {}, steps: [], expect: "advisory" };
  const step = { write: { "src/bad.ts": 'console.log("x");\n' }, file: "src/bad.ts" };
  return dispatchStep(sb, host, c, step, leg);
}

for (const leg of ["native", "bun"] satisfies Leg[]) {
  for (const host of ["claude", "codex"] as const) {
    test.concurrent(`fires while installed and is gated off when absent [${host}] [${leg}]`, async () => {
      using on = createSandbox({ git: true });
      using off = createSandbox({ git: true });
      const [fired, gated] = await Promise.all([
        violation(on, host, true, leg),
        violation(off, host, false, leg),
      ]);
      expect(fired.exitCode).toBe(0);
      expect(fired.stdout).toContain("Forbidden console.log");
      expect(gated).toMatchObject({ exitCode: 0, stdout: "" });
    });
  }
}
