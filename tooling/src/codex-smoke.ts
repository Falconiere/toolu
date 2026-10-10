/**
 * Exercises the real Codex plugin CLI against every plugin in this repository
 * inside a throwaway CODEX_HOME: register the marketplace, install the core
 * first and then every dependent, run each SessionStart command once the way
 * Codex does, remove everything, and check nothing is left installed.
 *
 * Env: CODEX_SMOKE_REPO (marketplace root), CODEX_SMOKE_TMP_ROOT (where the
 * home is created), CODEX_SMOKE_KEEP_HOME=1 (keep it for inspection).
 */
import { spawnSync } from "node:child_process";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  realpathSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { get, isNullish, list } from "./json-path.ts";
import { envOr } from "./env.ts";

const EXPECTED = { plugins: 12, sessionStart: 20 };
const LAUNCHER = /hooks\/dist\/|\bbun\b/;
const SESSION_START = '{"hook_event_name":"SessionStart","source":"startup"}\n';

class SmokeError extends Error {}

function fail(message: string): never {
  throw new SmokeError(message);
}

type Smoke = { root: string; home: string; project: string };

/** `CODEX_HOME=<home> codex <args>`, its stdout saved beside the home for inspection. */
function codex(smoke: Smoke, args: string[], save: string): string {
  const res = spawnSync("codex", args, {
    encoding: "utf8",
    env: { ...process.env, CODEX_HOME: smoke.home },
  });
  writeFileSync(join(smoke.home, save), res.stdout);
  if (res.status !== 0)
    fail(`codex ${args.join(" ")} exited ${String(res.status)}: ${res.stderr.trim()}`);
  return res.stdout;
}

function names(doc: unknown, key: string): string[] {
  return list(get(doc, key))
    .map((plugin) => String(get(plugin, "name")))
    .toSorted();
}

function expectedNames(root: string): string[] {
  return readdirSync(join(root, "plugins"))
    .map((dir) => join(root, "plugins", dir, ".codex-plugin/plugin.json"))
    .filter((manifest) => existsSync(manifest))
    .map((manifest) => String(get(JSON.parse(readFileSync(manifest, "utf8")), "name")))
    .toSorted();
}

function runSessionStart(smoke: Smoke, pluginRoot: string, argv: string[]): void {
  const res = spawnSync(argv[0] ?? "", argv.slice(1), {
    cwd: smoke.project,
    input: SESSION_START,
    encoding: "utf8",
    env: {
      ...process.env,
      CODEX_HOME: smoke.home,
      PLUGIN_ROOT: pluginRoot,
      CLAUDE_PLUGIN_ROOT: pluginRoot,
      TOOLU_HOST_OVERRIDE: "codex",
      TOOLU_PROJECT_DIR: smoke.project,
    },
  });
  if (res.status !== 0)
    fail(`SessionStart ${argv.join(" ")} exited ${String(res.status)}: ${res.stderr.trim()}`);
}

/** Script hooks run directly; launcher one-liners (#250) through sh -c, the way the host runs them. */
function sessionStarts(smoke: Smoke): number {
  let count = 0;
  for (const dir of readdirSync(join(smoke.root, "plugins")).toSorted()) {
    const pluginRoot = join(smoke.root, "plugins", dir);
    const file = join(pluginRoot, "hooks/hooks.json");
    if (!existsSync(file)) continue;
    const hooks = list(
      get(JSON.parse(readFileSync(file, "utf8")), "hooks", "SessionStart"),
    ).flatMap((group) => list(get(group, "hooks")));
    for (const hook of hooks) {
      const command = get(hook, "command");
      if (get(hook, "type") !== "command" || typeof command !== "string") continue;
      const windows = get(hook, "commandWindows");
      if (!isNullish(windows) || LAUNCHER.test(command)) {
        runSessionStart(smoke, pluginRoot, ["sh", "-c", command]);
      } else {
        const unquoted =
          command.startsWith('"') && command.endsWith('"') ? command.slice(1, -1) : command;
        const prefix = ["${CLAUDE_PLUGIN_ROOT}/", "${PLUGIN_ROOT}/"].find((p) =>
          unquoted.startsWith(p),
        );
        if (prefix === undefined) fail(`unsupported SessionStart command: ${command}`);
        const script = join(pluginRoot, unquoted.slice(prefix.length));
        if (!existsSync(script) || (statSync(script).mode & 0o111) === 0) {
          fail(`SessionStart command is not executable: ${script}`);
        }
        runSessionStart(smoke, pluginRoot, [script]);
      }
      count += 1;
    }
  }
  return count;
}

function installAll(smoke: Smoke, expected: string[]): void {
  const added = codex(
    smoke,
    ["plugin", "marketplace", "add", smoke.root, "--json"],
    "marketplace-add.json",
  );
  if (get(JSON.parse(added), "marketplaceName") !== "toolu")
    fail("marketplace registered under an unexpected name");
  const listed = codex(smoke, ["plugin", "marketplace", "list"], "marketplaces.txt");
  if (!/^toolu\s/m.test(listed)) fail("toolu marketplace is not listed");
  const available: unknown = JSON.parse(
    codex(smoke, ["plugin", "list", "--available", "--json"], "available.json"),
  );
  const availableCount = list(get(available, "available")).length;
  if (availableCount !== EXPECTED.plugins)
    fail(`expected ${String(EXPECTED.plugins)} available plugins, found ${String(availableCount)}`);
  if (names(available, "available").join() !== expected.join())
    fail("available plugin names differ from checked-in manifests");
  process.stdout.write(`codex-smoke: available=${String(availableCount)}\n`);
  // Install the core first so every dependent plugin observes the supported order.
  codex(smoke, ["plugin", "add", "toolu@toolu", "--json"], "install-toolu.json");
  for (const name of expected.filter((n) => n !== "toolu")) {
    codex(smoke, ["plugin", "add", `${name}@toolu`, "--json"], `install-${name}.json`);
  }
  const installed: unknown = JSON.parse(
    codex(smoke, ["plugin", "list", "--json"], "installed.json"),
  );
  const installedCount = list(get(installed, "installed")).length;
  if (installedCount !== EXPECTED.plugins)
    fail(`expected ${String(EXPECTED.plugins)} installed plugins, found ${String(installedCount)}`);
  if (names(installed, "installed").join() !== expected.join())
    fail("installed plugin names differ from checked-in manifests");
  process.stdout.write(`codex-smoke: installed=${String(installedCount)}\n`);
}

function removeAll(smoke: Smoke, expected: string[]): void {
  let removed = 0;
  for (const name of [...expected.filter((n) => n !== "toolu"), "toolu"]) {
    codex(smoke, ["plugin", "remove", `${name}@toolu`, "--json"], `remove-${name}.json`);
    removed += 1;
  }
  const after: unknown = JSON.parse(
    codex(smoke, ["plugin", "list", "--json"], "after-remove.json"),
  );
  if (list(get(after, "installed")).length !== 0)
    fail("plugins remain installed after removal smoke test");
  process.stdout.write(`codex-smoke: removed=${String(removed)}\n`);
}

function smokeHome(): { tmpRoot: string; home: string } {
  const tmpRoot = envOr("CODEX_SMOKE_TMP_ROOT", tmpdir());
  if (!existsSync(tmpRoot)) fail(`temporary root does not exist: ${tmpRoot}`);
  if (lstatSync(tmpRoot).isSymbolicLink()) fail(`temporary root must not be a symlink: ${tmpRoot}`);
  const real = realpathSync(tmpRoot);
  return { tmpRoot: real, home: mkdtempSync(join(real, "toolu-codex-smoke.")) };
}

/** Remove only what this run created: a toolu-codex-smoke.* directory directly under the temp root. */
function cleanup(tmpRoot: string, home: string): void {
  if (envOr("CODEX_SMOKE_KEEP_HOME", "0") !== "0") {
    process.stdout.write(`codex-smoke: preserved ${home}\n`);
    return;
  }
  if (realpathSync(dirname(home)) === tmpRoot && basename(home).startsWith("toolu-codex-smoke.")) {
    rmSync(home, { recursive: true, force: true });
  }
}

function main(): number {
  let created: { tmpRoot: string; home: string } | null = null;
  try {
    const root = envOr("CODEX_SMOKE_REPO", resolve(import.meta.dir, "../.."));
    if (Bun.which("codex") === null) fail("codex CLI is required");
    if (!existsSync(join(root, ".agents/plugins"))) fail(`not a Codex marketplace: ${root}`);
    created = smokeHome();
    const smoke: Smoke = { root, home: created.home, project: join(created.home, "project") };
    mkdirSync(smoke.project, { recursive: true });
    const expected = expectedNames(root);
    installAll(smoke, expected);
    const starts = sessionStarts(smoke);
    if (starts !== EXPECTED.sessionStart)
      fail(
        `expected ${String(EXPECTED.sessionStart)} SessionStart commands, ran ${String(starts)}`,
      );
    process.stdout.write(`codex-smoke: session-start=${String(starts)}\n`);
    removeAll(smoke, expected);
    return 0;
  } catch (err: unknown) {
    if (!(err instanceof SmokeError)) throw err;
    console.error(`codex-smoke: ${err.message}`);
    return 1;
  } finally {
    if (created !== null) cleanup(created.tmpRoot, created.home);
  }
}

if (import.meta.main) process.exitCode = main();
