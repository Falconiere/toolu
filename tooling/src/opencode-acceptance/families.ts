/**
 * Every acceptance check, in run order (#362): the host contract probes, the
 * generated surface, the existing live scenario arrays with the plugins each
 * proves, the live test files, and the checks added for this acceptance.
 */
import { PRETOOL_SCENARIOS } from "../opencode-host/scenarios-pretool.ts";
import { PERMISSIONS_SMOKE_SCENARIOS } from "../opencode-host/scenarios-permissions-smoke.ts";
import { POSTTOOL_SCENARIOS } from "../opencode-host/scenarios-posttool-smoke.ts";
import { TS_QUALITY_SCENARIOS } from "../opencode-host/scenarios-ts-quality-smoke.ts";
import { PYTHON_QUALITY_SCENARIOS } from "../opencode-host/scenarios-python-quality-smoke.ts";
import { RUST_QUALITY_SCENARIOS } from "../opencode-host/scenarios-rust-quality-smoke.ts";
import { AST_GREP_SCENARIOS } from "../opencode-host/scenarios-ast-grep-smoke.ts";
import { ENTRY_SCENARIOS } from "../opencode-host/scenarios-entry.ts";
import { STARTUP_SCENARIOS } from "../opencode-host/scenarios-startup.ts";
import { PATH_SCENARIOS } from "../opencode-host/scenarios-paths.ts";
import { INSTALL_SCENARIOS } from "../opencode-host/scenarios-install.ts";
import { BABYSIT_SCENARIOS } from "../opencode-host/scenarios-babysit.ts";
import { CLI_SCENARIOS } from "../opencode-host/scenarios-cli.ts";
import { QUICKSTART_SCENARIOS } from "../opencode-host/scenarios-docs.ts";
import { MIGRATION_SCENARIOS } from "../opencode-host/scenarios-docs-migration.ts";
import { MIGRATION_REFUSAL_SCENARIOS } from "../opencode-host/scenarios-docs-refusals.ts";
import { STATUS_SCENARIOS } from "../opencode-host/scenarios-status.ts";
import { PACKAGE_SCENARIOS } from "../opencode-host/scenarios-package.ts";
import { SCENARIOS } from "../opencode-host/scenarios.ts";
import type { ProbeResults } from "../opencode-host/schema.ts";
import { probeGeneratedSurface } from "../opencode-host/surface-probe.ts";
import { CONCURRENT_SCENARIOS, NAME_SCENARIOS } from "../opencode-host/scenarios-acceptance.ts";
import { BUDGET_CHECK } from "./budget.ts";
import { hostEvidence, scenarioChecks, type AcceptanceCheck } from "./checks.ts";
import { liveTestChecks } from "./live-tests.ts";

/** One check per contract probe: its live verdict and observations must equal the committed ones. */
function probeChecks(committed: ProbeResults): AcceptanceCheck[] {
  return SCENARIOS.map(({ run, ...meta }) => ({
    id: `probe.${meta.id}`,
    family: "probe",
    plugins: [],
    evidence: hostEvidence(),
    run: async (ctx) => {
      const observation = await run(ctx);
      const before = committed.probes.find((probe) => probe.id === meta.id);
      const pass = JSON.stringify(before) === JSON.stringify({ ...meta, ...observation });
      return {
        pass,
        observed: pass
          ? { verdict: observation.verdict }
          : { live: observation, committed: before ?? "missing from probe-results.json" },
      };
    },
  }));
}

const SURFACE_CHECK: AcceptanceCheck = {
  id: "surface.generated",
  family: "surface",
  plugins: "all",
  evidence: hostEvidence(),
  run: async (ctx) => ({
    pass: true,
    observed: await probeGeneratedSurface(ctx.bin, ctx.cacheRoot),
  }),
};

/** Scenario arrays that predate the acceptance, with the plugins they prove. */
function scenarioFamilies(): AcceptanceCheck[] {
  return [
    ...scenarioChecks("entry", ["toolu"], "none", ENTRY_SCENARIOS),
    ...scenarioChecks("startup", "all", "none", STARTUP_SCENARIOS),
    // The path scenario runs and asserts the published Jev helper without Bun on PATH.
    ...scenarioChecks(
      "paths",
      ["toolu", "jev"],
      "none",
      PATH_SCENARIOS.filter((scenario) => scenario.id === "entry.helper-env"),
    ),
    // The worktree scenario asserts ast-grep registry modules in the selected checkout only.
    ...scenarioChecks(
      "paths",
      ["toolu", "ast-grep"],
      "none",
      PATH_SCENARIOS.filter((scenario) => scenario.id === "entry.worktree-state"),
    ),
    ...scenarioChecks("install", ["toolu"], "none", INSTALL_SCENARIOS),
    ...scenarioChecks("babysit", ["pr-babysit"], "fixture", BABYSIT_SCENARIOS),
    ...scenarioChecks("cli", ["toolu"], "none", CLI_SCENARIOS),
    ...scenarioChecks("docs", ["toolu", "ast-grep"], "none", QUICKSTART_SCENARIOS),
    ...scenarioChecks("docs", ["toolu"], "none", MIGRATION_SCENARIOS),
    ...scenarioChecks("docs", ["toolu"], "none", MIGRATION_REFUSAL_SCENARIOS),
    ...scenarioChecks("status", ["statusline"], "none", STATUS_SCENARIOS),
    ...scenarioChecks("package", "all", "none", PACKAGE_SCENARIOS),
    ...scenarioChecks("pretool", ["toolu"], "none", PRETOOL_SCENARIOS),
    ...scenarioChecks("permissions", ["toolu"], "none", PERMISSIONS_SMOKE_SCENARIOS),
    ...scenarioChecks("posttool", ["toolu"], "none", POSTTOOL_SCENARIOS),
    ...scenarioChecks("ts-quality", ["ts-quality"], "none", TS_QUALITY_SCENARIOS),
    ...scenarioChecks("python-quality", ["python-quality"], "none", PYTHON_QUALITY_SCENARIOS),
    ...scenarioChecks("rust-quality", ["rust-quality"], "none", RUST_QUALITY_SCENARIOS),
    ...scenarioChecks("ast-grep", ["ast-grep"], "none", AST_GREP_SCENARIOS),
  ];
}

/** The whole registry, given the committed probe results the contract probes compare against. */
export function acceptanceChecks(committed: ProbeResults): AcceptanceCheck[] {
  return [
    ...probeChecks(committed),
    SURFACE_CHECK,
    ...scenarioChecks("surface", "all", "none", NAME_SCENARIOS),
    ...scenarioFamilies(),
    ...liveTestChecks(),
    ...scenarioChecks("concurrent", ["toolu"], "none", CONCURRENT_SCENARIOS),
    BUDGET_CHECK,
  ];
}
