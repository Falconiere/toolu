/**
 * One OpenCode acceptance run (#362): preflight, the pinned host, a warm-up
 * that records the SDK the host provisioned, every selected check in order,
 * the regression controls, the live external services, and the report.
 */
import { mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { warmUp } from "../opencode-host-probe.ts";
import { hostCacheDir, resolveHostBinary, type HostBinary } from "../opencode-host/install.ts";
import { contractPaths } from "../opencode-host/results.ts";
import { packTarball } from "../opencode-host/scenarios-entry.ts";
import {
  ContractError,
  PinSchema,
  ProbeResultsSchema,
  readJson,
  type Pin,
  type ProbeResults,
} from "../opencode-host/schema.ts";
import {
  catalogNames,
  coverage,
  hostEvidence,
  inSequence,
  selectChecks,
  type AcceptanceCheck,
  type AcceptanceContext,
} from "./checks.ts";
import { runControls, selectControls } from "./controls.ts";
import { acceptanceChecks } from "./families.ts";
import { LIVE_TEST_FILES, runExternal, type ExternalResult } from "./live-tests.ts";
import { preflight } from "./preflight.ts";
import {
  acceptancePass,
  type AcceptanceReport,
  type CheckResult,
  type ControlResult,
} from "./report.ts";

/**
 * Service credentials no fixture check may see. An external test that names
 * one in `requires` gets it back; the others (context7 runs without a key)
 * never do.
 */
const SERVICE_KEYS = [
  "CONTEXT7_API_KEY",
  "EXA_API_KEY",
  "TYPESAFE_API_KEY",
  "JIRA_API_TOKEN",
  "JIRA_PAT",
  "JIRA_EMAIL",
  "JIRA_BASE_URL",
];

type AcceptanceOptions = { only: readonly string[] };

function errorText(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

/** Remove the service credentials from `env` (this process by default); the removed values are returned for the external tests. */
export function setAsideKeys(
  env: Record<string, string | undefined> = process.env,
): Record<string, string> {
  const kept: Record<string, string> = {};
  for (const key of SERVICE_KEYS) {
    const value = env[key];
    if (value !== undefined && value !== "") kept[key] = value;
    delete env[key];
  }
  return kept;
}

/** Put the set-aside credentials back once the run is over, so a later run in this process gets them again. */
export function restoreKeys(
  keys: Readonly<Record<string, string>>,
  env: Record<string, string | undefined> = process.env,
): void {
  for (const [key, value] of Object.entries(keys)) env[key] = value;
}

/** A package override left in the shell would point every shim at another package; only controls set it. */
export function refusePresetPackage(env: Record<string, string | undefined> = process.env): void {
  const preset = env.TOOLU_ACCEPTANCE_PACKAGE;
  if (preset !== undefined && preset !== "")
    throw new ContractError(
      `unset TOOLU_ACCEPTANCE_PACKAGE (${preset}): only regression controls set it`,
    );
}

async function runCheck(check: AcceptanceCheck, ctx: AcceptanceContext): Promise<CheckResult> {
  const { id, family, evidence } = check;
  const started = performance.now();
  const plugins: CheckResult["plugins"] = check.plugins === "all" ? "all" : [...check.plugins];
  const base = { id, family, plugins, evidence };
  try {
    const { pass, observed } = await check.run(ctx);
    return {
      ...base,
      status: pass ? "pass" : "fail",
      observed,
      durationMs: performance.now() - started,
    };
  } catch (err) {
    const error = errorText(err);
    return {
      ...base,
      status: "fail",
      observed: {},
      error,
      durationMs: performance.now() - started,
    };
  }
}

function printResult(result: CheckResult): void {
  const verdict = result.status === "pass" ? "pass" : "FAIL";
  const detail = result.error ?? JSON.stringify(result.observed);
  process.stdout.write(`${result.id} ${verdict} ${Math.round(result.durationMs)}ms ${detail}\n`);
}

/** The committed host versions must match the live host; drift means the evidence is stale. */
function hostVersions(committed: ProbeResults, host: HostBinary, provisioned: string): CheckResult {
  const observed = {
    cliVersion: `${committed.host.cliVersion} -> ${host.version}`,
    provisionedSdkVersion: `${committed.host.provisionedSdkVersion} -> ${provisioned}`,
  };
  const pass =
    committed.host.cliVersion === host.version &&
    committed.host.provisionedSdkVersion === provisioned;
  const check = {
    id: "probe.host-versions",
    family: "probe",
    plugins: [],
    evidence: hostEvidence(),
  };
  return { ...check, status: pass ? "pass" : "fail", observed, durationMs: 0 };
}

function runChecks(
  checks: readonly AcceptanceCheck[],
  ctx: AcceptanceContext,
): Promise<CheckResult[]> {
  return inSequence(checks, async (check) => {
    const result = await runCheck(check, ctx);
    printResult(result);
    return result;
  });
}

function runExternals(keys: Record<string, string>): Promise<ExternalResult[]> {
  const tests = LIVE_TEST_FILES.flatMap((entry) =>
    (entry.external ?? []).map((test) => ({ entry, test })),
  );
  return inSequence(tests, ({ entry, test }) => runExternal(entry, test, keys));
}

type HostSetup = {
  pin: Pin;
  host: HostBinary;
  ctx: AcceptanceContext;
  provisioned: string;
  work: string;
};

async function setUpHost(): Promise<HostSetup> {
  const pin = readJson(contractPaths().pin, PinSchema);
  const host = await resolveHostBinary(pin);
  const cacheRoot = join(hostCacheDir(pin), "run-cache");
  mkdirSync(cacheRoot, { recursive: true });
  const provisioned = await warmUp({ bin: host.bin, cacheRoot });
  const work = mkdtempSync(join(tmpdir(), "toolu-acceptance-"));
  try {
    const ctx = { bin: host.bin, cacheRoot, tarball: packTarball(work) };
    return { pin, host, provisioned, work, ctx };
  } catch (err) {
    rmSync(work, { recursive: true, force: true });
    throw err;
  }
}

type Collected = {
  complete: boolean;
  checks: readonly AcceptanceCheck[];
  results: CheckResult[];
  controls: ControlResult[];
  external: ExternalResult[];
  scrubbed: string[];
  tools: Record<string, string>;
};

/** Checks and controls of a run; `only` names either kind, and an unknown name is an error. */
export function selection(
  registry: readonly AcceptanceCheck[],
  only: readonly string[],
): { checks: AcceptanceCheck[]; controls: ReturnType<typeof selectControls> } {
  const controls = selectControls(only);
  const checkIds = only.filter((id) => !controls.some((control) => control.id === id));
  if (only.length > 0 && checkIds.length === 0) return { checks: [], controls };
  return { checks: selectChecks(registry, checkIds), controls };
}

function buildReport(setup: HostSetup, startedAt: Date, run: Collected): AcceptanceReport {
  const outcomes = run.results.flatMap((result) => {
    const check = run.checks.find((item) => item.id === result.id);
    return check === undefined ? [] : [{ check, pass: result.status === "pass" }];
  });
  const report: AcceptanceReport = {
    version: 1,
    complete: run.complete,
    pass: false,
    startedAt: startedAt.toISOString(),
    durationMs: Date.now() - startedAt.getTime(),
    host: {
      cli: setup.pin.cli.package,
      cliVersion: setup.host.version,
      sdk: setup.pin.sdk.package,
      provisionedSdkVersion: setup.provisioned,
      platform: `${process.platform}-${process.arch}`,
      bun: Bun.version,
      installSource: setup.host.installSource,
    },
    tools: run.tools,
    scrubbed: run.scrubbed,
    checks: run.results,
    controls: run.controls,
    external: run.external,
    coverage: coverage(outcomes, catalogNames()),
  };
  return { ...report, pass: acceptancePass(report) };
}

/** Run the selected checks (all of them when `only` is empty) and build the report. */
export async function runAcceptance(opts: AcceptanceOptions): Promise<AcceptanceReport> {
  const startedAt = new Date();
  const committed = readJson(contractPaths().results, ProbeResultsSchema);
  const registry = acceptanceChecks(committed);
  const { checks, controls } = selection(registry, opts.only);
  refusePresetPackage();
  const keys = setAsideKeys();
  try {
    return await runSelected({
      startedAt,
      committed,
      registry,
      checks,
      controls,
      keys,
      complete: opts.only.length === 0,
    });
  } finally {
    restoreKeys(keys);
  }
}

type RunPlan = {
  startedAt: Date;
  committed: ProbeResults;
  registry: readonly AcceptanceCheck[];
  checks: readonly AcceptanceCheck[];
  controls: ReturnType<typeof selectControls>;
  keys: Record<string, string>;
  complete: boolean;
};

async function runSelected(plan: RunPlan): Promise<AcceptanceReport> {
  const tools = await preflight();
  if (tools.missing.length > 0)
    throw new ContractError(`required tools missing:\n  ${tools.missing.join("\n  ")}`);
  const setup = await setUpHost();
  try {
    const versions = hostVersions(plan.committed, setup.host, setup.provisioned);
    printResult(versions);
    const results = [versions, ...(await runChecks(plan.checks, setup.ctx))];
    const controlResults = await runControls(plan.controls, plan.registry, setup.ctx);
    const external = plan.complete ? await runExternals(plan.keys) : [];
    return buildReport(setup, plan.startedAt, {
      complete: plan.complete,
      checks: plan.checks,
      results,
      controls: controlResults,
      external,
      scrubbed: Object.keys(plan.keys).toSorted(),
      tools: tools.tools,
    });
  } finally {
    rmSync(setup.work, { recursive: true, force: true });
  }
}
