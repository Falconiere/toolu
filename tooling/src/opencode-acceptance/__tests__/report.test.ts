import { expect, test } from "bun:test";
import { hostEvidence } from "../checks.ts";
import {
  AcceptanceReportSchema,
  acceptancePass,
  markdownSummary,
  selectedPass,
  uncovered,
  type AcceptanceReport,
  type CheckResult,
} from "../report.ts";

// The verdict and summary over report data shaped like a real run (#362 AC-4, AC-5).

function check(
  id: string,
  status: "pass" | "fail",
  service: "none" | "fixture" = "none",
): CheckResult {
  return {
    id,
    family: id.split(".")[0] ?? id,
    plugins: ["toolu"],
    evidence: hostEvidence(service),
    status,
    durationMs: 1200,
    observed: { ready: 1 },
  };
}

function report(overrides: Partial<AcceptanceReport> = {}): AcceptanceReport {
  return {
    version: 1,
    complete: true,
    pass: true,
    startedAt: "2026-10-04T20:00:00.000Z",
    durationMs: 480_000,
    host: {
      cli: "opencode-ai",
      cliVersion: "1.18.34",
      sdk: "@opencode-ai/plugin",
      provisionedSdkVersion: "1.18.34",
      platform: "darwin-arm64",
      bun: "1.4.2",
      installSource: "npm:opencode-ai@1.18.34 (bun add --exact)",
    },
    tools: { git: "git version 2.50.0" },
    scrubbed: ["TYPESAFE_API_KEY"],
    checks: [check("entry.npm-root", "pass"), check("jev.transport", "pass", "fixture")],
    controls: [
      {
        id: "control.missing-post-tool",
        regression: "x",
        check: "posttool.edit",
        detected: true,
        observed: {},
      },
    ],
    external: [
      {
        id: "external.jev",
        service: "api.typesafe.ai",
        execution: "in-process",
        status: "not-configured",
        detail: "TYPESAFE_API_KEY is not set",
      },
    ],
    coverage: { toolu: ["entry.npm-root"], jev: ["jev.transport"] },
    ...overrides,
  };
}

test.concurrent("a complete, passing, fully covered run is acceptance", () => {
  const passing = report();
  expect(AcceptanceReportSchema.parse(passing)).toEqual(passing);
  expect(acceptancePass(passing)).toBe(true);
});

test.concurrent("a failed check, a missed control, a coverage gap or a narrowed run is not", () => {
  expect(acceptancePass(report({ checks: [check("entry.npm-root", "fail")] }))).toBe(false);
  const missed = {
    id: "control.v2-entry",
    regression: "x",
    check: "entry.local-shim",
    observed: {},
  };
  expect(acceptancePass(report({ controls: [{ ...missed, detected: false }] }))).toBe(false);
  expect(acceptancePass(report({ coverage: { toolu: ["entry.npm-root"], "ast-grep": [] } }))).toBe(
    false,
  );
  expect(uncovered({ toolu: ["a"], "ast-grep": [] })).toEqual(["ast-grep"]);
  const narrowed = report({ complete: false });
  expect(acceptancePass(narrowed)).toBe(false);
  expect(selectedPass(narrowed)).toBe(true);
});

test.concurrent("an unavailable or unconfigured external service never changes the verdict", () => {
  const external: AcceptanceReport["external"] = [
    {
      id: "external.jev",
      service: "api.typesafe.ai",
      execution: "in-process",
      status: "unavailable",
      detail: "failed in 3s",
    },
  ];
  expect(acceptancePass(report({ external }))).toBe(true);
});

test.concurrent("the summary lists host versions and the three evidence categories apart", () => {
  const text = markdownSummary(report());
  expect(text).toContain("## OpenCode acceptance — darwin-arm64: PASS");
  expect(text).toContain(
    "`opencode-ai@1.18.34`, provisioned `@opencode-ai/plugin@1.18.34`, Bun 1.4.2",
  );
  expect(text).toContain("**Actual-host execution:** 2 checks");
  expect(text).toContain("**Fixture transport:**");
  expect(text).toContain("1 checks also used a loopback service fixture");
  expect(text).toContain("| api.typesafe.ai | not-configured | TYPESAFE_API_KEY is not set |");
  expect(text).toContain("| control.missing-post-tool | posttool.edit | detected |");
  expect(text).not.toContain("### Failures");
});

test.concurrent("the summary names every failure of a complete run", () => {
  const text = markdownSummary(
    report({
      pass: false,
      checks: [{ ...check("entry.npm-root", "fail"), error: "host timed out" }],
      coverage: { toolu: [], "ast-grep": [] },
    }),
  );
  expect(text).toContain(": FAIL");
  expect(text).toContain("- `entry.npm-root`: host timed out");
  expect(text).toContain("- `ast-grep` has no passing dedicated check");
});

test.concurrent("a narrowed run lists its failed checks but no coverage gaps", () => {
  const text = markdownSummary(
    report({
      pass: false,
      complete: false,
      checks: [{ ...check("entry.npm-root", "fail"), error: "host timed out" }],
      coverage: { toolu: [], "ast-grep": [] },
    }),
  );
  expect(text).toContain("narrowed run (not acceptance)");
  expect(text).toContain("- `entry.npm-root`: host timed out");
  expect(text).not.toContain("has no passing dedicated check");
});
