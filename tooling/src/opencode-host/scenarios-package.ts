/**
 * Live clean-install scenario (#361) on the pinned host.
 *
 * `npm pack` builds the tarball through the package's own prepack, the same
 * way the release publishes it. A sandbox with no checkout path in its env
 * (`TOOLU_REPO_ROOT` and `TOOLU_ROOT` blank) loads it through the npm route
 * with all 12 plugins selected. The agent's bash runs helpers from the
 * installed tree. The scenario then reads the installed tree itself: it must
 * equal the tarball, sit outside the checkout, own every published helper
 * symlink, and import every export.
 */
import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, realpathSync } from "node:fs";
import { join, relative } from "node:path";
import { z } from "zod";
import { listPluginManifests } from "../../../tools/toolu-opencode/src/inventory/scan.ts";
import { runHost, toolStates } from "./host-run.ts";
import {
  GATED_FILES,
  ROOT,
  diagnostics,
  entrySession,
  npmSpec,
  type EntryContext,
  type EntryResult,
  type EntryScenario,
} from "./scenarios-entry.ts";
import { DATA_ROOT, HELPERS, MODULES, modules } from "./scenarios-startup.ts";
import { ContractError } from "./schema.ts";

const ENV_BYTES = "SECRET=1\n";
const Exports = z.looseObject({ exports: z.record(z.string(), z.string()) });

function helperCommand(): string {
  return [
    `printf '%s\\n%s\\n' "$TOOLU_OPENCODE_ROOT" "$TOOLU_PLUGIN_ROOT" > roots.txt`,
    `"$TOOLU_BUN" --no-env-file "$TOOLU_PLUGIN_ROOT_STATUSLINE/hooks/dist/status.js" > status.txt 2>&1; echo "status=$?" >> markers.txt`,
    `"$TOOLU_BUN" "$TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR/scripts/report.ts" "$PWD/epic-status.json" brainstorm > report.txt 2>&1; echo "report=$?" >> markers.txt`,
    `"$TOOLU_BUN" "$TOOLU_PLUGIN_ROOT/hooks/dist/plan-ledger.js" path > ledger.txt 2>&1; echo "ledger=$?" >> markers.txt`,
  ].join("\n");
}

/** Every file under `dir`, relative and sorted, skipping the installer's `node_modules`. */
function treeFiles(dir: string, base = dir): string[] {
  return readdirSync(dir, { withFileTypes: true })
    .flatMap((entry) => {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) return entry.name === "node_modules" ? [] : treeFiles(path, base);
      return [relative(base, path)];
    })
    .toSorted();
}

/** The tarball's file list, without npm's `package/` prefix. */
function tarballFiles(tarball: string): string[] {
  const res = spawnSync("tar", ["-tzf", tarball], { encoding: "utf8" });
  if (res.status !== 0) throw new ContractError(`tar -tzf ${tarball}: ${res.stderr}`);
  return res.stdout
    .split("\n")
    .filter((line) => line !== "" && !line.endsWith("/"))
    .map((line) => line.replace(/^package\//, ""))
    .toSorted();
}

/** How many of the installed package's `exports` targets import from the installed tree. */
async function importable(installed: string): Promise<{ total: number; loaded: number }> {
  const manifest = Exports.parse(JSON.parse(readFileSync(join(installed, "package.json"), "utf8")));
  const targets = Object.values(manifest.exports);
  const results = await Promise.all(
    targets.map((target) =>
      import(join(installed, target)).then(
        () => true,
        () => false,
      ),
    ),
  );
  return { total: targets.length, loaded: results.filter(Boolean).length };
}

/** The published helper symlinks that resolve inside the installed package. */
function ownedHelpers(project: string, installed: string): number {
  const root = realpathSync(installed);
  return HELPERS.filter((helper) => {
    const path = join(project, DATA_ROOT, helper);
    return existsSync(path) && realpathSync(path).startsWith(`${root}/`);
  }).length;
}

function read(dir: string, rel: string): string {
  const path = join(dir, rel);
  return existsSync(path) ? readFileSync(path, "utf8") : "";
}

/** What the installed tree and the agent's helper runs show; empty roots mean bash never ran. */
async function installedObservations(project: string, tarball: string) {
  const [installed = "", pluginRoot = ""] = read(project, "roots.txt").split("\n");
  const exists = installed !== "" && existsSync(installed);
  const imports = exists ? await importable(installed) : { total: 0, loaded: 0 };
  return {
    outsideCheckout: exists && !realpathSync(installed).startsWith(`${realpathSync(ROOT)}/`),
    pluginRoot: pluginRoot === join(installed, "plugins/toolu"),
    treeMatchesTarball:
      exists && treeFiles(installed).join("\n") === tarballFiles(tarball).join("\n"),
    helpersOwned: exists ? ownedHelpers(project, installed) : 0,
    exportsLoaded: imports.loaded,
    exportsTotal: imports.total,
    markers: read(project, "markers.txt").trim().split("\n").join(","),
    statusReady: read(project, "status.txt").includes("toolu: ready"),
    reported: read(project, "report.txt").includes("reported brainstorm"),
  };
}

async function cleanInstall(ctx: EntryContext): Promise<EntryResult> {
  const names = (listPluginManifests(join(ROOT, "plugins")) ?? []).map((p) => p.name);
  using s = entrySession(ctx, {
    files: {
      ...GATED_FILES,
      ".opencode/toolu/plugins.json": JSON.stringify({ version: 1, enabled: names }),
    },
    config: () => ({ plugin: [npmSpec(ctx.tarball)], permission: { bash: "allow" } }),
    scripts: (project) => ({
      "package.clean": [
        { tool: "write", args: { filePath: join(project, ".env"), content: "PWNED\n" } },
        { tool: "bash", args: { command: helperCommand(), description: "helpers" } },
      ],
    }),
  });
  s.env.TOOLU_REPO_ROOT = "";
  s.env.TOOLU_ROOT = "";
  const hostRun = await runHost(ctx.bin, s, ["--print-logs", "PROBE:package.clean"]);
  const write = toolStates(hostRun).find((state) => state.tool === "write");
  const observed = {
    plugins: names.length,
    ready: diagnostics(
      hostRun.stderr,
      `toolu: ready (${names.length} plugins, ${MODULES.length + HELPERS.length} startup artifacts)`,
    ),
    notReady: diagnostics(hostRun.stderr, "toolu: not ready"),
    modules: modules(s).toSorted().join(",") === MODULES.toSorted().join(","),
    writeDenied: write?.status === "error" && /protected/i.test(write.error ?? ""),
    envUnchanged: s.sb.read(".env") === ENV_BYTES,
    ...(await installedObservations(s.sb.project, ctx.tarball)),
  };
  const pass =
    observed.plugins === 12 &&
    observed.ready === 1 &&
    observed.notReady === 0 &&
    observed.modules &&
    observed.writeDenied &&
    observed.envUnchanged &&
    observed.outsideCheckout &&
    observed.pluginRoot &&
    observed.treeMatchesTarball &&
    observed.helpersOwned === HELPERS.length &&
    observed.exportsTotal > 0 &&
    observed.exportsLoaded === observed.exportsTotal &&
    observed.markers === "status=0,report=0,ledger=0" &&
    observed.statusReady &&
    observed.reported;
  return { pass, observed };
}

export const PACKAGE_SCENARIOS: EntryScenario[] = [
  {
    id: "package.clean-install",
    claim:
      "The npm-packed tarball loads all 12 plugins with no checkout env, and every helper, export and installed file comes from the tarball",
    run: cleanInstall,
  },
];
