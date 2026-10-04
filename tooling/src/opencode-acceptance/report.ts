/**
 * The OpenCode acceptance report (#362): what ran, on which host and platform,
 * how each check's evidence was produced, and the verdict. CI uploads it, and
 * the Markdown summary goes to the job summary.
 */
import { z } from "zod";

const Evidence = z.object({
  execution: z.enum(["actual-host", "in-process"]),
  model: z.literal("scripted-loopback"),
  service: z.enum(["none", "fixture", "live"]),
});
const Observed = z.record(z.string(), z.unknown());

const CheckResultSchema = z.object({
  id: z.string(),
  family: z.string(),
  plugins: z.union([z.array(z.string()), z.literal("all")]),
  evidence: Evidence,
  status: z.enum(["pass", "fail"]),
  durationMs: z.number(),
  observed: Observed,
  error: z.string().optional(),
});
export type CheckResult = z.infer<typeof CheckResultSchema>;

const ControlResult = z.object({
  id: z.string(),
  regression: z.string(),
  check: z.string(),
  detected: z.boolean(),
  observed: Observed,
  error: z.string().optional(),
});
type ControlResult = z.infer<typeof ControlResult>;

const External = z.object({
  id: z.string(),
  service: z.string(),
  execution: z.literal("in-process"),
  status: z.enum(["available", "unavailable", "not-configured"]),
  detail: z.string(),
});

export const AcceptanceReportSchema = z.object({
  version: z.literal(1),
  complete: z.boolean(),
  pass: z.boolean(),
  startedAt: z.string(),
  durationMs: z.number(),
  host: z.object({
    cli: z.string(),
    cliVersion: z.string(),
    sdk: z.string(),
    provisionedSdkVersion: z.string(),
    platform: z.string(),
    bun: z.string(),
    installSource: z.string(),
  }),
  tools: z.record(z.string(), z.string()),
  /** Service credentials removed from the run's environment so no fixture check can reach a live service. */
  scrubbed: z.array(z.string()),
  checks: z.array(CheckResultSchema),
  controls: z.array(ControlResult),
  external: z.array(External),
  coverage: z.record(z.string(), z.array(z.string())),
});
export type AcceptanceReport = z.infer<typeof AcceptanceReportSchema>;

/** Catalog plugins with no passing dedicated actual-host check. */
export function uncovered(coverage: Record<string, string[]>): string[] {
  return Object.entries(coverage)
    .filter(([, ids]) => ids.length === 0)
    .map(([name]) => name);
}

/** Selected work passed: every check passed and every control was detected. */
export function selectedPass(report: Pick<AcceptanceReport, "checks" | "controls">): boolean {
  return (
    report.checks.every((check) => check.status === "pass") &&
    report.controls.every((control) => control.detected)
  );
}

/** Acceptance: a complete run whose selected work passed and which covers every catalog plugin. */
export function acceptancePass(
  report: Pick<AcceptanceReport, "complete" | "checks" | "controls" | "coverage">,
): boolean {
  return report.complete && selectedPass(report) && uncovered(report.coverage).length === 0;
}

function seconds(ms: number): string {
  return `${(ms / 1000).toFixed(1)} s`;
}

function familyRows(checks: readonly CheckResult[]): string[] {
  const families = [...new Set(checks.map((check) => check.family))];
  return families.map((family) => {
    const rows = checks.filter((check) => check.family === family);
    const passed = rows.filter((check) => check.status === "pass").length;
    const services = [...new Set(rows.map((check) => check.evidence.service))].join(", ");
    const time = rows.reduce((total, check) => total + check.durationMs, 0);
    return `| ${family} | ${passed}/${rows.length} | ${services} | ${seconds(time)} |`;
  });
}

function evidenceLines(report: AcceptanceReport): string[] {
  const host = report.checks.filter((check) => check.evidence.execution === "actual-host");
  const fixture = report.checks.filter((check) => check.evidence.service === "fixture");
  return [
    `- **Actual-host execution:** ${host.length} checks ran the pinned \`opencode\` binary.`,
    `- **Fixture transport:** every model reply came from the scripted loopback provider; ${fixture.length} checks also used a loopback service fixture or bare git remote.`,
    `- **Live external services:** reported below, never part of the verdict.`,
  ];
}

function controlRows(report: AcceptanceReport): string[] {
  return report.controls.map(
    (control) =>
      `| ${control.id} | ${control.check} | ${control.detected ? "detected" : "**missed**"} |`,
  );
}

function failureLines(report: AcceptanceReport): string[] {
  const checks = report.checks
    .filter((check) => check.status === "fail")
    .map((check) => `- \`${check.id}\`: ${check.error ?? JSON.stringify(check.observed)}`);
  const controls = report.controls
    .filter((control) => !control.detected)
    .map((control) => `- \`${control.id}\` was not detected by \`${control.check}\``);
  // A narrowed run selects few checks on purpose; only a complete run can have a coverage gap.
  const gaps = report.complete
    ? uncovered(report.coverage).map((name) => `- \`${name}\` has no passing dedicated check`)
    : [];
  return [...checks, ...controls, ...gaps];
}

/** The Markdown job summary; the three evidence categories are listed apart. */
export function markdownSummary(report: AcceptanceReport): string {
  const { host } = report;
  const verdict = report.pass ? "PASS" : "FAIL";
  const scope = report.complete ? "complete run" : "narrowed run (not acceptance)";
  const failures = failureLines(report);
  return [
    `## OpenCode acceptance — ${host.platform}: ${verdict}`,
    "",
    `\`${host.cli}@${host.cliVersion}\`, provisioned \`${host.sdk}@${host.provisionedSdkVersion}\`, Bun ${host.bun}, ${scope}, ${seconds(report.durationMs)}.`,
    "",
    ...evidenceLines(report),
    "",
    "| Family | Pass | Service | Time |",
    "|---|---|---|---|",
    ...familyRows(report.checks),
    "",
    "| Regression control | Check | Result |",
    "|---|---|---|",
    ...controlRows(report),
    "",
    "| Live external service | Status | Detail |",
    "|---|---|---|",
    ...report.external.map((item) => `| ${item.service} | ${item.status} | ${item.detail} |`),
    "",
    ...(failures.length > 0 ? ["### Failures", "", ...failures, ""] : []),
  ].join("\n");
}
