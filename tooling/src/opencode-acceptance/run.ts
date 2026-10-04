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
import { packTarball, ROOT } from "../opencode-host/scenarios-entry.ts";
import {
  ContractError,
  PinSchema,
  ProbeResultsSchema,
  readJson,
  type Pin,
  type ProbeResults,
} from "../opencode-host/schema.ts";
import { listPluginManifests } from "../../../tools/toolu-opencode/src/inventory/scan.ts";
import {
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

/** Service credentials no fixture check may see; only the matching external test gets its key back. */
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

/** Remove the service credentials from this process; the removed values are returned for the external tests. */
function setAsideKeys(): Record<string, string> {
  const kept: Record<string, string> = {};
  for (const key of SERVICE_KEYS) {
    const value = process.env[key];
    if (value !== undefined && value !== "") kept[key] = value;
    delete process.env[key];
  }
  return kept;
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
  const ctx = { bin: host.bin, cacheRoot, tarball: packTarball(work) };
  return { pin, host, provisioned, work, ctx };
}

/** The catalog every acceptance run must cover; an unreadable or empty catalog fails closed. */
export function catalogNames(pluginsRoot: string = join(ROOT, "plugins")): string[] {
  const names = (listPluginManifests(pluginsRoot) ?? []).map((manifest) => manifest.name);
  if (names.length === 0) throw new ContractError(`no catalog plugins under ${pluginsRoot}`);
  return names;
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
function selection(
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
  const keys = setAsideKeys();
  const tools = await preflight();
  if (tools.missing.length > 0)
    throw new ContractError(`required tools missing:\n  ${tools.missing.join("\n  ")}`);
  const setup = await setUpHost();
  try {
    const versions = hostVersions(committed, setup.host, setup.provisioned);
    printResult(versions);
    const results = [versions, ...(await runChecks(checks, setup.ctx))];
    const controlResults = await runControls(controls, registry, setup.ctx);
    const complete = opts.only.length === 0;
    const external = complete ? await runExternals(keys) : [];
    return buildReport(setup, startedAt, {
      complete,
      checks,
      results,
      controls: controlResults,
      external,
      scrubbed: Object.keys(keys).toSorted(),
      tools: tools.tools,
    });
  } finally {
    rmSync(setup.work, { recursive: true, force: true });
  }
}
