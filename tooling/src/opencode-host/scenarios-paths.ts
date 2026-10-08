/**
 * Live path and environment scenarios (#343) on the pinned host.
 *
 * - `entry.helper-env`: the project and the plugin catalog sit under paths with
 *   spaces and `bun` is not on PATH. The agent's bash still runs the published
 *   Jev helper, which execs `toolu jev`, a toolu core
 *   CLI through `$TOOLU_PLUGIN_ROOT`, and finds a leaf plugin's own root.
 * - `entry.worktree-state`: a main checkout and its linked worktree enable
 *   different plugins. Each keeps its own data root, and the main checkout's
 *   failing gate refuses its own commit without being consumed.
 */
import {
  cpSync,
  existsSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runHost, toolStates } from "./host-run.ts";
import type { Scripts } from "./provider.ts";
import {
  PROJECT_FILES,
  ROOT,
  diagnostics,
  entrySession,
  installShim,
  type EntryContext,
  type EntryResult,
  type EntryScenario,
} from "./scenarios-entry.ts";
import { ContractError } from "./schema.ts";

const DATA_ROOT = ".opencode/toolu/state";
const SELECTION = ".opencode/toolu/plugins.json";
const GATE = ".opencode/tmp/quality-gate-status.json";
const GATE_BYTES = JSON.stringify({ status: "failing", reason: "main checkout is failing" });
const AST_GREP_MODULES = [
  "pre-tools.d/ast-grep@toolu__search-nudge.js",
  "post-tools.d/ast-grep@toolu__byte-savings.js",
];

function selection(names: readonly string[]): string {
  return JSON.stringify({ version: 1, enabled: names });
}

function git(cwd: string, args: string[]): void {
  const res = spawnSync("git", ["-C", cwd, ...args], { encoding: "utf8" });
  if (res.status !== 0) throw new ContractError(`git ${args.join(" ")}: ${res.stderr.trim()}`);
}

function helperScript(): Scripts {
  const command = [
    `TYPESAFE_API_KEY= "$TOOLU_CONFIG_DIR/jev/jev.sh" choice q -s '{}' -o a -o b > jev.txt 2>&1; echo "jev=$?" >> markers.txt`,
    `bun "$TOOLU_PLUGIN_ROOT/hooks/dist/plan-ledger.js" path > ledger.txt 2>&1; echo "ledger=$?" >> markers.txt`,
    `test -f "$TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR/scripts/report.ts"; echo "epic=$?" >> markers.txt`,
    `printf %s "$HOME" > home.txt`,
  ].join("\n");
  return { "paths.helpers": [{ tool: "bash", args: { command, description: "helpers" } }] };
}

function read(dir: string, rel: string): string {
  const path = join(dir, rel);
  return existsSync(path) ? readFileSync(path, "utf8") : "";
}

async function helperEnv(ctx: EntryContext): Promise<EntryResult> {
  const catalog = mkdtempSync(join(tmpdir(), "toolu catalog "));
  try {
    cpSync(join(ROOT, "plugins"), join(catalog, "plugins"), { recursive: true });
    using s = entrySession(ctx, {
      config: () => ({ permission: { bash: "allow" } }),
      scripts: helperScript(),
    });
    const project = join(s.sb.root, "my project");
    git(s.sb.root, ["clone", "-q", s.sb.project, project]);
    cpSync(join(s.sb.project, "opencode.json"), join(project, "opencode.json"));
    writeFileSync(join(project, SELECTION), selection(["jev", "epic-orchestrator"]));
    installShim(s, project);
    s.env.TOOLU_REPO_ROOT = catalog;
    s.env.PATH = `${join(ROOT, "target/debug")}:/usr/bin:/bin`;
    const hostRun = await runHost(
      ctx.bin,
      s,
      ["--print-logs", "PROBE:paths.helpers"],
      undefined,
      project,
    );
    const bash = toolStates(hostRun).find((state) => state.tool === "bash");
    const markers = read(project, "markers.txt");
    const observed = {
      ready: diagnostics(hostRun.stderr, "toolu: ready"),
      bashRan: bash?.status === "completed",
      jevWrapper: read(project, "jev.txt").includes("jev: TYPESAFE_API_KEY unset"),
      markers: markers.trim().split("\n").join(","),
      ledgerPath: read(project, "ledger.txt").includes(join(project, ".opencode")),
      home: read(project, "home.txt") === s.sb.home,
      helperInDataRoot: existsSync(join(project, DATA_ROOT, "jev/jev.sh")),
    };
    const pass =
      observed.ready === 1 &&
      observed.bashRan &&
      observed.jevWrapper &&
      // An unset key exits 1 before parsing or transport; 127 means the helper was not found.
      observed.markers === "jev=1,ledger=0,epic=0" &&
      observed.ledgerPath &&
      observed.home &&
      observed.helperInDataRoot;
    return { pass, observed };
  } finally {
    rmSync(catalog, { recursive: true, force: true });
  }
}

function modules(dir: string): string[] {
  return ["pre-tools.d", "post-tools.d"].flatMap((sub) => {
    const path = join(dir, DATA_ROOT, "toolu", sub);
    return existsSync(path) ? readdirSync(path).map((file) => `${sub}/${file}`) : [];
  });
}

const WORKTREE_SCRIPTS: Scripts = {
  "paths.commit": [
    {
      tool: "bash",
      args: { command: "git commit --allow-empty -m 'fix: worktree'", description: "commit" },
    },
  ],
  "paths.touch": [{ tool: "bash", args: { command: "touch allowed.txt", description: "touch" } }],
};

async function worktreeState(ctx: EntryContext): Promise<EntryResult> {
  using s = entrySession(ctx, {
    config: () => ({ permission: { bash: "allow" } }),
    scripts: WORKTREE_SCRIPTS,
    files: { ...PROJECT_FILES, [SELECTION]: selection(["toolu", "ast-grep"]), [GATE]: GATE_BYTES },
  });
  const main = s.sb.project;
  const worktree = join(s.sb.root, "linked wt");
  git(main, ["worktree", "add", "-q", worktree, "-b", "wt"]);
  cpSync(join(main, "opencode.json"), join(worktree, "opencode.json"));
  writeFileSync(join(worktree, SELECTION), selection(["toolu"]));
  rmSync(join(worktree, GATE));
  installShim(s);
  installShim(s, worktree);
  s.env.TOOLU_REPO_ROOT = ROOT;
  const inMain = await runHost(ctx.bin, s, ["--print-logs", "PROBE:paths.commit"]);
  const inWorktree = await runHost(
    ctx.bin,
    s,
    ["--print-logs", "PROBE:paths.touch"],
    undefined,
    worktree,
  );
  const commit = toolStates(inMain).find((state) => state.tool === "bash");
  const touch = toolStates(inWorktree).find((state) => state.tool === "bash");
  const observed = {
    mainReady: diagnostics(inMain.stderr, "toolu: ready (2 plugins"),
    worktreeReady: diagnostics(inWorktree.stderr, "toolu: ready (1 plugins"),
    commitDenied:
      commit?.status === "error" && (commit.error ?? "").includes("main checkout is failing"),
    gateUnchanged: read(main, GATE) === GATE_BYTES,
    mainModulesKept: AST_GREP_MODULES.every((module) => modules(main).includes(module)),
    worktreeOwnRoot: existsSync(join(worktree, DATA_ROOT, "toolu")),
    worktreeNoAstGrep: !modules(worktree).some((module) => module.includes("ast-grep@toolu__")),
    worktreeBashRan: touch?.status === "completed" && existsSync(join(worktree, "allowed.txt")),
  };
  return {
    pass: Object.values(observed).every((value) => value === true || value === 1),
    observed,
  };
}

export const PATH_SCENARIOS: EntryScenario[] = [
  {
    id: "entry.helper-env",
    claim:
      "With spaces in the project and catalog paths and no bun on PATH, bash runs a published Jev helper, a core CLI and finds a plugin root",
    run: helperEnv,
  },
  {
    id: "entry.worktree-state",
    claim:
      "A linked worktree keeps its own data root, and the main checkout's failing gate refuses only its own commit, unchanged",
    run: worktreeState,
  },
];
