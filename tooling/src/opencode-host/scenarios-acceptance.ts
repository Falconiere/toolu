/**
 * Live checks added by the OpenCode acceptance run (#362).
 *
 * `surface.discovered-names` reads the skills the pinned host discovered
 * through toolu's `config` hook and holds every toolu skill to the documented
 * name rule: the host itself loads invalid names without complaint (probe
 * `surface.names`). `concurrent.sessions` runs three `opencode run` processes
 * at once over two projects: each enforces on its own, and one project's
 * failing gate refuses only its own commit.
 */
import { readFileSync, realpathSync } from "node:fs";
import { basename, dirname, join, sep } from "node:path";
import { z } from "zod";
import { RUN_TIMEOUT_MS, runHost, toolStates } from "./host-run.ts";
import { skills } from "./install-host.ts";
import type { Scripts } from "./provider.ts";
import {
  GATED_FILES,
  ROOT,
  acceptancePackageDir,
  diagnostics,
  entrySession,
  installShim,
  type EntryContext,
  type EntryResult,
  type EntryScenario,
} from "./scenarios-entry.ts";
import type { ProbeSession } from "./session.ts";

/** The documented skill name rule: lowercase alphanumerics in single-hyphen groups, at most 64 characters. */
const NAME_RULE = /^[a-z0-9]+(-[a-z0-9]+)*$/;
const MAX_NAME = 64;
const GATE = ".opencode/tmp/quality-gate-status.json";
const GATE_BYTES = JSON.stringify({ status: "failing", reason: "project Q is failing" });
const CONFIG = JSON.stringify({
  version: 1,
  gates: {
    protectedFiles: { mode: "block" },
    qualityGate: { mode: "block" },
    commitGate: { mode: "off" },
    pushReview: { mode: "off" },
  },
});
const Catalog = z.object({
  plugins: z.array(z.looseObject({ skills: z.array(z.object({ id: z.string() })) })),
});

/** A project loading toolu through the shim, every installed plugin selected (no selection file). */
function shimmed(
  ctx: EntryContext,
  files: Record<string, string>,
  scripts: Scripts | ((project: string) => Scripts),
): ProbeSession {
  const s = entrySession(ctx, {
    config: () => ({ permission: { bash: "allow" } }),
    files: { ...GATED_FILES, ...files },
    scripts,
  });
  installShim(s);
  s.env.TOOLU_REPO_ROOT = ROOT;
  return s;
}

/** Skill ids in the catalog of the package under test. */
function catalogSkills(packageDir: string): Set<string> {
  const text = readFileSync(join(packageDir, "generated/opencode.toolu.json"), "utf8");
  const catalog = Catalog.parse(JSON.parse(text));
  return new Set(catalog.plugins.flatMap((plugin) => plugin.skills.map((skill) => skill.id)));
}

function nameProblem(name: string, location: string): boolean {
  return !NAME_RULE.test(name) || name.length > MAX_NAME || basename(dirname(location)) !== name;
}

async function discoveredNames(ctx: EntryContext): Promise<EntryResult> {
  const packageDir = realpathSync(acceptancePackageDir());
  const expected = catalogSkills(packageDir);
  using s = shimmed(ctx, {}, {});
  const { rows, log } = await skills(ctx, s);
  const prefix = `${join(packageDir, "generated/skills")}${sep}`;
  const owned = rows.filter((row) => row.location.startsWith(prefix));
  const invalid = owned.filter((row) => nameProblem(row.name, row.location)).map((row) => row.name);
  const missing = [...expected].filter((id) => !rows.some((row) => row.name === id));
  // toolu validates its own catalog and refuses to start on a bad id; the name rule covers what loads.
  const observed = {
    ready: diagnostics(log, "toolu: ready"),
    owned: owned.length,
    catalog: expected.size,
    invalid: invalid.join(","),
    missing: missing.join(","),
  };
  const pass =
    observed.ready === 1 &&
    expected.size > 0 &&
    owned.length === expected.size &&
    invalid.length === 0 &&
    missing.length === 0;
  return { pass, observed };
}

function writeEnv(project: string): { tool: string; args: Record<string, unknown> } {
  return { tool: "write", args: { filePath: join(project, ".env"), content: "PWNED\n" } };
}

function bash(command: string): { tool: string; args: Record<string, unknown> } {
  return { tool: "bash", args: { command, description: command } };
}

const P_SCRIPTS = (project: string): Scripts => ({
  "concurrent.one": [
    writeEnv(project),
    bash("touch one.txt"),
    bash("git commit --allow-empty -m 'fix: one'"),
  ],
  "concurrent.two": [writeEnv(project), bash("touch two.txt")],
});
const Q_SCRIPTS = (project: string): Scripts => ({
  "concurrent.q": [writeEnv(project), bash("touch q.txt && git commit --allow-empty -m 'fix: q'")],
});

type HostRun = Awaited<ReturnType<typeof runHost>>;

function enforced(run: HostRun): boolean {
  const write = toolStates(run).find((state) => state.tool === "write");
  return (
    diagnostics(run.stderr, "toolu: ready") === 1 &&
    write?.status === "error" &&
    /protected/i.test(write.error ?? "")
  );
}

/**
 * The host keeps its session database in the profile, and a second process
 * starting on the same profile at the same moment fails with "database is
 * locked". So the second run in P comes from another profile (P2, a second
 * user or terminal): both share P's project, its gate files and toolu's state.
 */
async function concurrentSessions(ctx: EntryContext): Promise<EntryResult> {
  const files = { ".opencode/toolu.config.json": CONFIG };
  using p = shimmed(ctx, files, P_SCRIPTS);
  // P2 only lends its profile (HOME, XDG dirs): it runs with cwd = P's project, whose shim loads toolu.
  using p2 = entrySession(ctx, { files: {} });
  p2.env.TOOLU_REPO_ROOT = ROOT;
  using q = shimmed(ctx, { ...files, [GATE]: GATE_BYTES }, Q_SCRIPTS);
  const envBytes = GATED_FILES[".env"];
  const [one, two, inQ] = await Promise.all([
    runHost(ctx.bin, p, ["--print-logs", "PROBE:concurrent.one"]),
    runHost(ctx.bin, p2, ["--print-logs", "PROBE:concurrent.two"], RUN_TIMEOUT_MS, p.sb.project),
    runHost(ctx.bin, q, ["--print-logs", "PROBE:concurrent.q"]),
  ]);
  const commitDenied = toolStates(inQ).some(
    (state) =>
      state.tool === "bash" && state.status === "error" && /quality gate/i.test(state.error ?? ""),
  );
  const observed = {
    eachEnforced: [one, two, inQ].every(enforced),
    envUnchanged: p.sb.read(".env") === envBytes && q.sb.read(".env") === envBytes,
    markersInP: p.exists("one.txt") && p.exists("two.txt"),
    pCommitted: p.sb.git("log", "--format=%s").split("\n").includes("fix: one"),
    qCommitDenied: commitDenied && !q.exists("q.txt"),
    qNotCommitted: !q.sb.git("log", "--format=%s").split("\n").includes("fix: q"),
    qGateUnchanged: q.sb.read(GATE) === GATE_BYTES,
  };
  return { pass: Object.values(observed).every(Boolean), observed };
}

export const NAME_SCENARIOS: EntryScenario[] = [
  {
    id: "surface.discovered-names",
    claim:
      "Every toolu skill the host discovers through the config hook follows the documented name rule",
    run: discoveredNames,
  },
];

export const CONCURRENT_SCENARIOS: EntryScenario[] = [
  {
    id: "concurrent.sessions",
    claim:
      "Three concurrent runs over two projects each enforce, and a failing gate refuses only its own project's commit",
    run: concurrentSessions,
  },
];
