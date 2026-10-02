/**
 * Live entry scenarios (#336): the pinned host loads `@toolu/opencode` itself —
 * through its npm route (a packed tarball), a local `.opencode/plugins/` shim, and
 * a config entry naming the package directory — and toolu's hooks take effect.
 *
 * Each scenario runs real `opencode run` sessions in an isolated profile against
 * the scripted provider. Load counts come from toolu's own readiness diagnostic
 * in the host log (`--print-logs`); enforcement is read from the tool states and
 * from the files on disk.
 */
import { cpSync, mkdirSync, readdirSync, symlinkSync } from "node:fs";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { run } from "@toolu/conformance/harness/spawn";
import { stagePlugins } from "../../../tools/toolu-opencode/scripts/bundle-plugins.ts";
import { runHost, toolStates } from "./host-run.ts";
import type { Scripts } from "./provider.ts";
import type { ScenarioContext } from "./scenario.ts";
import { ContractError } from "./schema.ts";
import { openSession, type ProbeSession, type SessionOptions } from "./session.ts";

export const ROOT = resolve(import.meta.dir, "../../..");
const PACKAGE_DIR = join(ROOT, "tools/toolu-opencode");
const PACKAGE_ENTRIES = ["package.json", "README.md", "LICENSE", "src", "generated"];
const PACK_TIMEOUT_MS = 120_000;
const ENV_BYTES = "SECRET=1\n";

export type EntryContext = ScenarioContext & { tarball: string };
type Observed = Record<string, boolean | number | string>;
export type EntryResult = { pass: boolean; observed: Observed };
export type EntryScenario = {
  id: string;
  claim: string;
  run: (ctx: EntryContext) => Promise<EntryResult>;
};

/** A project with a protected `.env` and only the core plugin enabled. */
export const PROJECT_FILES = {
  ".env": ENV_BYTES,
  ".opencode/toolu.config.json": JSON.stringify({
    version: 1,
    gates: { protectedFiles: { mode: "block" } },
  }),
  ".opencode/toolu/plugins.json": JSON.stringify({ version: 1, enabled: ["toolu"] }),
};

/** Pack the package the way npm publishes it: sources plus the staged plugin catalog. */
export async function packTarball(workDir: string): Promise<string> {
  const stage = join(workDir, "package");
  mkdirSync(stage, { recursive: true });
  for (const entry of PACKAGE_ENTRIES)
    cpSync(join(PACKAGE_DIR, entry), join(stage, entry), { recursive: true });
  stagePlugins(join(ROOT, "plugins"), join(stage, "plugins"));
  const res = await run(
    [process.execPath, "pm", "pack", "--ignore-scripts", "--destination", workDir],
    { cwd: stage, timeoutMs: PACK_TIMEOUT_MS },
  );
  const tarball = readdirSync(workDir).find((name) => name.endsWith(".tgz"));
  if (res.exitCode !== 0 || tarball === undefined)
    throw new ContractError(`bun pm pack failed (exit ${res.exitCode}): ${res.stderr.trim()}`);
  return join(workDir, tarball);
}

function npmSpec(tarball: string): string {
  return `@toolu/opencode@file:${tarball}`;
}

/** `.opencode/plugins/toolu.ts` re-exporting the package root, resolved through `node_modules`. */
export function installShim(session: ProbeSession): void {
  session.sb.write(".opencode/plugins/toolu.ts", 'export { default } from "@toolu/opencode";\n');
  mkdirSync(join(session.sb.project, "node_modules/@toolu"), { recursive: true });
  symlinkSync(PACKAGE_DIR, join(session.sb.project, "node_modules/@toolu/opencode"));
}

function writeEnvScript(project: string): Scripts {
  return {
    "entry.write-env": [
      { tool: "write", args: { filePath: join(project, ".env"), content: "PWNED\n" } },
      { tool: "bash", args: { command: "touch allowed.txt", description: "allowed" } },
    ],
  };
}

export const TOUCH_SCRIPT: Scripts = {
  "entry.touch": [{ tool: "bash", args: { command: "touch denied.txt", description: "x" } }],
};

export function entrySession(ctx: EntryContext, opts: SessionOptions): ProbeSession {
  const opened = openSession(ctx.cacheRoot, { files: PROJECT_FILES, ...opts });
  // Bootstrap resolves Bun from TOOLU_BUN; the isolated HOME has no ~/.bun.
  opened.env.TOOLU_BUN = process.execPath;
  return opened;
}

/** Count of host-log lines carrying toolu's diagnostic `message`. */
export function diagnostics(stderr: string, prefix: string): number {
  return stderr.split("\n").filter((line) => line.includes(`message="${prefix}`)).length;
}

async function protectedWrite(ctx: EntryContext, s: ProbeSession): Promise<Observed> {
  const hostRun = await runHost(ctx.bin, s, ["--print-logs", "PROBE:entry.write-env"]);
  const states = toolStates(hostRun);
  const write = states.find((state) => state.tool === "write");
  const bash = states.find((state) => state.tool === "bash");
  return {
    ready: diagnostics(hostRun.stderr, "toolu: ready"),
    notReady: diagnostics(hostRun.stderr, "toolu: not ready"),
    duplicate: diagnostics(hostRun.stderr, "toolu: duplicate load skipped"),
    writeDenied: write?.status === "error" && /protected/i.test(write.error ?? ""),
    envUnchanged: s.sb.read(".env") === ENV_BYTES,
    bashRan: bash?.status === "completed" && s.exists("allowed.txt"),
  };
}

function enforced(observed: Observed): boolean {
  return (
    observed.notReady === 0 &&
    observed.writeDenied === true &&
    observed.envUnchanged === true &&
    observed.bashRan === true
  );
}

async function npmRoot(ctx: EntryContext): Promise<EntryResult> {
  using s = entrySession(ctx, {
    config: () => ({ plugin: [npmSpec(ctx.tarball)], permission: { bash: "allow" } }),
    scripts: writeEnvScript,
  });
  const observed = await protectedWrite(ctx, s);
  return { pass: observed.ready === 1 && enforced(observed), observed };
}

async function localShim(ctx: EntryContext): Promise<EntryResult> {
  using s = entrySession(ctx, {
    config: () => ({ permission: { bash: "allow" } }),
    scripts: writeEnvScript,
  });
  installShim(s);
  s.env.TOOLU_REPO_ROOT = ROOT;
  const observed = await protectedWrite(ctx, s);
  return { pass: observed.ready === 1 && enforced(observed), observed };
}

async function bothRoutes(ctx: EntryContext): Promise<EntryResult> {
  using s = entrySession(ctx, {
    config: () => ({ plugin: [npmSpec(ctx.tarball)], permission: { bash: "allow" } }),
    scripts: writeEnvScript,
  });
  installShim(s);
  s.env.TOOLU_REPO_ROOT = ROOT;
  const observed = await protectedWrite(ctx, s);
  const once = observed.ready === 1 && observed.duplicate === 1;
  return { pass: once && enforced(observed), observed };
}

async function initFailure(ctx: EntryContext): Promise<EntryResult> {
  using s = entrySession(ctx, {
    config: (root) => ({
      plugin: [[pathToFileURL(PACKAGE_DIR).href, { repoRoot: join(root, "missing") }]],
      permission: { bash: "allow" },
    }),
    scripts: TOUCH_SCRIPT,
  });
  const hostRun = await runHost(ctx.bin, s, ["--print-logs", "PROBE:entry.touch"]);
  const bash = toolStates(hostRun).find((state) => state.tool === "bash");
  const observed = {
    ready: diagnostics(hostRun.stderr, "toolu: ready"),
    notReady: diagnostics(hostRun.stderr, "toolu: not ready"),
    bashDenied: bash?.status === "error" && (bash.error ?? "").startsWith("toolu: not ready"),
    markerAbsent: !s.exists("denied.txt"),
  };
  const pass =
    observed.ready === 0 && observed.notReady === 1 && observed.bashDenied && observed.markerAbsent;
  return { pass, observed };
}

export const ENTRY_SCENARIOS: EntryScenario[] = [
  {
    id: "entry.npm-root",
    claim: "The npm route loads the packed root export once and its gates refuse a protected write",
    run: npmRoot,
  },
  {
    id: "entry.local-shim",
    claim: "A .opencode/plugins shim re-exporting @toolu/opencode loads once and enforces",
    run: localShim,
  },
  {
    id: "entry.both-routes",
    claim: "With the npm spec and a local shim, one instance enforces and the other is skipped",
    run: bothRoutes,
  },
  {
    id: "entry.init-failure",
    claim: "A config entry whose setup fails denies every tool call instead of failing open",
    run: initFailure,
  },
];
