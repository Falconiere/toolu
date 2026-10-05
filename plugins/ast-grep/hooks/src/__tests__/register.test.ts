/**
 * AC-4 (#268), ported from register.bats: the real SessionStart `register`
 * bundle, behind its hooks.json launcher, publishes exactly search-nudge and
 * byte-savings into each host's registry, silently; prunes only ast-grep's
 * stale entries and aged tmp residue; refreshes a drifted copy; is byte- and
 * mtime-stable when re-run; and both modules fire through toolu's dispatcher
 * bundles only while ast-grep is installed.
 *
 * register.bats, all 13: pre- and post-tools sync, the Codex root, no tmp
 * leftovers and an empty stdout are "publishes exactly the two bundles";
 * stale-entry pruning and aged tmp residue are "prunes only its own"; drift is
 * "refreshes"; idempotence is "a second run"; the four dispatcher end-to-end
 * tests are the installed/absent pairs at the bottom, on both hosts.
 */
import { expect, test } from "bun:test";
import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  statSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { launchedArgv } from "@toolu/conformance/harness/entry-command";
import { toStdin } from "@toolu/conformance/harness/fixtures";
import { runPostBundle } from "@toolu/conformance/harness/posttool";
import { installPlugins, pretoolEnv, runBundle } from "@toolu/conformance/harness/pretool";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch } from "@toolu/conformance/harness/spawn";
import { PLUGIN_ROOT } from "./golden-sandbox.ts";

const NUDGE = "ast-grep@toolu__search-nudge.js";
const SAVINGS = "ast-grep@toolu__byte-savings.js";
const dist = (entry: string) => join(PLUGIN_ROOT, "hooks/dist", `${entry}.js`);

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
  return run(launchedArgv({ plugin: "ast-grep", event: "SessionStart", entry: "register" }), {
    cwd: sb.project,
    env: { HOME: sb.home, CLAUDE_PLUGIN_ROOT: PLUGIN_ROOT, TOOLU_BUN: process.execPath, ...env },
    stdin: "{}",
  });
}

const preDir = (root: string) => join(root, "toolu", "pre-tools.d");
const postDir = (root: string) => join(root, "toolu", "post-tools.d");

for (const root of ROOTS) {
  test.concurrent(`publishes exactly the two bundles under the ${root.name} root, silently`, async () => {
    using sb = createSandbox();
    expect(await register(sb, root.env(sb))).toMatchObject({ exitCode: 0, stdout: "", stderr: "" });
    const base = root.dir(sb);
    expect(readdirSync(preDir(base))).toEqual([NUDGE]);
    expect(readdirSync(postDir(base))).toEqual([SAVINGS]);
    expect(readFileSync(join(preDir(base), NUDGE))).toEqual(readFileSync(dist("search-nudge")));
    expect(readFileSync(join(postDir(base), SAVINGS))).toEqual(readFileSync(dist("byte-savings")));
    if (root.name !== "Claude") expect(existsSync(join(sb.home, ".claude", "toolu"))).toBe(false);
  });
}

test.concurrent("prunes only its own stale entries and aged tmp residue", async () => {
  using sb = createSandbox();
  const base = join(sb.home, ".claude");
  const put = (dir: string, name: string, ageMs = 0) => {
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, name), "x\n");
    const when = new Date(Date.now() - ageMs);
    utimesSync(join(dir, name), when, when);
  };
  put(preDir(base), "ast-grep@toolu__search-nudge.sh");
  put(preDir(base), "ast-grep@toolu__removed-module.sh");
  put(preDir(base), "other@market__keep.sh");
  put(preDir(base), "ast-grep@toolu__search-nudge.js.tmp.111", 120_000);
  put(preDir(base), "ast-grep@toolu__fresh-module.js.tmp.222", 5_000);
  put(preDir(base), "other@market__keep.sh.tmp.333", 120_000);
  put(postDir(base), "ast-grep@toolu__byte-savings.sh");
  expect((await register(sb)).exitCode).toBe(0);
  expect(readdirSync(preDir(base)).toSorted()).toEqual([
    "ast-grep@toolu__fresh-module.js.tmp.222",
    NUDGE,
    "other@market__keep.sh",
    "other@market__keep.sh.tmp.333",
  ]);
  expect(readdirSync(postDir(base))).toEqual([SAVINGS]);
});

test.concurrent("refreshes a registry copy that drifted from its bundle", async () => {
  using sb = createSandbox();
  const file = join(preDir(join(sb.home, ".claude")), NUDGE);
  mkdirSync(preDir(join(sb.home, ".claude")), { recursive: true });
  writeFileSync(file, "stale content\n");
  expect((await register(sb)).exitCode).toBe(0);
  expect(readFileSync(file)).toEqual(readFileSync(dist("search-nudge")));
});

test.concurrent("a second run leaves both modules byte- and mtime-stable", async () => {
  using sb = createSandbox();
  const base = join(sb.home, ".claude");
  const files = [join(preDir(base), NUDGE), join(postDir(base), SAVINGS)];
  expect((await register(sb)).exitCode).toBe(0);
  const past = new Date(Date.now() - 60_000);
  for (const file of files) utimesSync(file, past, past);
  const before = files.map((file) => statSync(file).mtimeMs);
  expect(await register(sb)).toMatchObject({ exitCode: 0, stdout: "" });
  expect(files.map((file) => statSync(file).mtimeMs)).toEqual(before);
  expect(readdirSync(preDir(base))).toEqual([NUDGE]);
});

/** Register ast-grep for `host` with it installed or not, as the host would see it. */
async function seed(sb: Sandbox, host: "claude" | "codex", installed: boolean): Promise<void> {
  const specs = installed ? ["toolu@toolu", "ast-grep@toolu"] : ["toolu@toolu"];
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
}

/** A structural Grep through the pre-tools bundle. */
async function structuralGrep(sb: Sandbox, host: "claude" | "codex", installed: boolean) {
  await seed(sb, host, installed);
  const fixture = {
    kind: "tool",
    event: "PreToolUse",
    toolName: "Grep",
    toolInput: { pattern: "fn handle_request", glob: "*.rs" },
  } as const;
  const stdin = JSON.stringify(toStdin(host, fixture, { cwd: sb.project }));
  return runBundle({ cwd: sb.project, env: pretoolEnv(sb, host), stdin });
}

/** A ranged Read of a 4000-byte file through the post-tools bundle; returns the session's ledger path. */
async function rangedRead(sb: Sandbox, host: "claude" | "codex", installed: boolean) {
  await seed(sb, host, installed);
  const payload = {
    session_id: "e2e",
    cwd: sb.project,
    hook_event_name: "PostToolUse",
    tool_name: "Read",
    tool_input: { file_path: sb.write("big.txt", "x".repeat(4000)) },
    tool_response: "abc",
  };
  const res = await runPostBundle(sb, {
    cwd: sb.project,
    env: pretoolEnv(sb, host),
    stdin: JSON.stringify(payload),
  });
  const root = host === "codex" ? sb.codexHome : join(sb.home, ".claude");
  return { res, ledger: join(root, "toolu", "byte-savings", "e2e.jsonl") };
}

for (const host of ["claude", "codex"] as const) {
  test.concurrent(`search-nudge fires while installed and is gated off when absent [${host}]`, async () => {
    using on = createSandbox({ git: true });
    using off = createSandbox({ git: true });
    const [fired, gated] = await Promise.all([
      structuralGrep(on, host, true),
      structuralGrep(off, host, false),
    ]);
    expect(fired.exitCode).toBe(0);
    expect(fired.stdout).toContain("ast-grep");
    expect(gated).toMatchObject({ exitCode: 0, stdout: "" });
  });

  test.concurrent(`byte-savings records while installed and is gated off when absent [${host}]`, async () => {
    using on = createSandbox({ git: true });
    using off = createSandbox({ git: true });
    const [fired, gated] = await Promise.all([
      rangedRead(on, host, true),
      rangedRead(off, host, false),
    ]);
    expect(fired.res).toMatchObject({ exitCode: 0, stdout: "" });
    expect(readFileSync(fired.ledger, "utf8")).toBe('{"kind":"read","returned":3,"full":4000}\n');
    expect(gated.res).toMatchObject({ exitCode: 0, stdout: "" });
    expect(existsSync(gated.ledger)).toBe(false);
  });
}
