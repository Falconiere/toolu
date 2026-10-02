/**
 * Live startup scenarios (#342): the pinned host loads `@toolu/opencode` through a
 * local shim and toolu runs every selected plugin's startup. Readiness is read
 * from toolu's host-log diagnostic, enforcement from the tool states, and the
 * contributions from the project's data root on disk.
 */
import { cpSync, existsSync, mkdtempSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { listPluginManifests } from "../../../tools/toolu-opencode/src/inventory/scan.ts";
import { runHost, toolStates } from "./host-run.ts";
import type { Scripts } from "./provider.ts";
import {
  PROJECT_FILES,
  ROOT,
  TOUCH_SCRIPT,
  diagnostics,
  entrySession,
  installShim,
  type EntryContext,
  type EntryResult,
  type EntryScenario,
} from "./scenarios-entry.ts";
import type { ProbeSession } from "./session.ts";

/** Where the shimmed package bootstraps a project that sets no data-root override. */
const DATA_ROOT = ".opencode/toolu/state";

const ALLOWED_SCRIPT: Scripts = {
  "startup.touch": [{ tool: "bash", args: { command: "touch allowed.txt", description: "x" } }],
};

function selection(names: readonly string[]): string {
  return JSON.stringify({ version: 1, enabled: names });
}

function shimmedSession(
  ctx: EntryContext,
  names: readonly string[],
  scripts: Scripts,
  repoRoot: string,
): ProbeSession {
  const files = { ...PROJECT_FILES, ".opencode/toolu/plugins.json": selection(names) };
  const s = entrySession(ctx, {
    config: () => ({ permission: { bash: "allow" } }),
    scripts,
    files,
  });
  installShim(s);
  s.env.TOOLU_REPO_ROOT = repoRoot;
  return s;
}

/** Registry modules on disk, as `<dir>/<file>`. */
function modules(s: ProbeSession): string[] {
  return ["pre-tools.d", "post-tools.d"].flatMap((dir) => {
    const path = join(s.sb.project, DATA_ROOT, "toolu", dir);
    return existsSync(path) ? readdirSync(path).map((file) => `${dir}/${file}`) : [];
  });
}

const HELPERS = [
  "agent-browser/agent-browser.sh",
  "context7/search.sh",
  "exa-search/search.sh",
  "jev/jev.sh",
  "jira/jira.sh",
  "statusline/statusline.sh",
  "toolu-review/write-state.sh",
];

async function fullStartup(ctx: EntryContext): Promise<EntryResult> {
  const names = (listPluginManifests(join(ROOT, "plugins")) ?? []).map((p) => p.name);
  using s = shimmedSession(ctx, names, ALLOWED_SCRIPT, ROOT);
  const hostRun = await runHost(ctx.bin, s, ["--print-logs", "PROBE:startup.touch"]);
  const bash = toolStates(hostRun).find((state) => state.tool === "bash");
  const observed = {
    plugins: names.length,
    ready: diagnostics(
      hostRun.stderr,
      `toolu: ready (${names.length} plugins, 12 startup artifacts)`,
    ),
    notReady: diagnostics(hostRun.stderr, "toolu: not ready"),
    modules: modules(s).length,
    helpers: HELPERS.filter((path) => existsSync(join(s.sb.project, DATA_ROOT, path))).length,
    bashRan: bash?.status === "completed" && s.exists("allowed.txt"),
  };
  const pass =
    observed.plugins === 16 &&
    observed.ready === 1 &&
    observed.notReady === 0 &&
    observed.modules === 5 &&
    observed.helpers === HELPERS.length &&
    observed.bashRan;
  return { pass, observed };
}

async function startupFailure(ctx: EntryContext): Promise<EntryResult> {
  const catalog = mkdtempSync(join(tmpdir(), "toolu-startup-catalog-"));
  try {
    cpSync(join(ROOT, "plugins"), join(catalog, "plugins"), { recursive: true });
    rmSync(join(catalog, "plugins/ts-quality/hooks/dist/post-tool-use.js"));
    using s = shimmedSession(ctx, ["toolu", "ts-quality"], TOUCH_SCRIPT, catalog);
    const hostRun = await runHost(ctx.bin, s, ["--print-logs", "PROBE:entry.touch"]);
    const bash = toolStates(hostRun).find((state) => state.tool === "bash");
    const observed = {
      ready: diagnostics(hostRun.stderr, "toolu: ready"),
      notReady: diagnostics(hostRun.stderr, "toolu: not ready: bootstrap: ts-quality/register: "),
      bashDenied: bash?.status === "error" && (bash.error ?? "").startsWith("toolu: not ready"),
      fileAbsent: !s.exists("denied.txt"),
    };
    const pass =
      observed.ready === 0 && observed.notReady === 1 && observed.bashDenied && observed.fileAbsent;
    return { pass, observed };
  } finally {
    rmSync(catalog, { recursive: true, force: true });
  }
}

async function startupDisable(ctx: EntryContext): Promise<EntryResult> {
  using s = shimmedSession(ctx, ["toolu", "ast-grep"], ALLOWED_SCRIPT, ROOT);
  const first = await runHost(ctx.bin, s, ["--print-logs", "PROBE:startup.touch"]);
  const before = modules(s).filter((module) => module.includes("ast-grep@toolu__")).length;
  s.sb.write(".opencode/toolu/plugins.json", selection(["toolu"]));
  const second = await runHost(ctx.bin, s, ["--print-logs", "PROBE:startup.touch"]);
  const observed = {
    firstReady: diagnostics(first.stderr, "toolu: ready (2 plugins"),
    secondReady: diagnostics(second.stderr, "toolu: ready (1 plugins"),
    astGrepBefore: before,
    astGrepAfter: modules(s).filter((module) => module.includes("ast-grep@toolu__")).length,
  };
  const pass =
    observed.firstReady === 1 &&
    observed.secondReady === 1 &&
    observed.astGrepBefore === 2 &&
    observed.astGrepAfter === 0;
  return { pass, observed };
}

export const STARTUP_SCENARIOS: EntryScenario[] = [
  {
    id: "entry.full-startup",
    claim:
      "With all 16 plugins enabled, every startup runs and its modules and helpers are in place",
    run: fullStartup,
  },
  {
    id: "entry.startup-failure",
    claim:
      "A selected plugin whose registration cannot complete leaves toolu not ready and refuses tools",
    run: startupFailure,
  },
  {
    id: "entry.startup-disable",
    claim: "Disabling a plugin removes its registry modules at the next startup",
    run: startupDisable,
  },
];
