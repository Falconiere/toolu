/** GitHub Actions evidence for the same budgets shown in the bench:shell log. */
import { appendFileSync } from "node:fs";
import type { ShellBench } from "./bench-shell.ts";

type Budget = {
  bundleBytes: number;
  writesBundleBytes: number;
  togetherBundleBytes: number;
  coldStartMs: number;
  parseP99Us: number;
};

const status = (value: number, limit: number) => (value <= limit ? "pass" : "**over budget**");

export function appendJobSummary(bench: ShellBench, budget: Budget, hardColdStart: boolean): void {
  const path = process.env.GITHUB_STEP_SUMMARY;
  if (!path) return;
  const { machine: m, bundle: b, coldStart: c, parse: p } = bench;
  const coldStatus = `${hardColdStart ? "hard" : "report-only"}: ${status(c.deltaP50, budget.coldStartMs)}`;
  appendFileSync(
    path,
    [
      "## Shell analysis budget",
      "",
      `Bun ${m.bun}; ${m.platform} ${m.arch}; ${m.cpu}; ${m.date}. Cold-start is hard on macOS Apple Silicon or with \`TOOLU_LATENCY_ENFORCE=1\`; otherwise it is report-only. Bundle sizes and parser p99 are hard everywhere.`,
      "",
      "| Measure | Result | Budget | Policy and status |",
      "| --- | ---: | ---: | --- |",
      `| Shell bundle, unminified | +${b.deltaBytes} B | ${budget.bundleBytes} B | hard: ${status(b.deltaBytes, budget.bundleBytes)} |`,
      `| Writes bundle, unminified | +${b.writesDeltaBytes} B | ${budget.writesBundleBytes} B | hard: ${status(b.writesDeltaBytes, budget.writesBundleBytes)} |`,
      `| Both entries, unminified | +${b.togetherDeltaBytes} B | ${budget.togetherBundleBytes} B | hard: ${status(b.togetherDeltaBytes, budget.togetherBundleBytes)} |`,
      `| Cold-start p50 delta, shipped unminified | +${c.deltaP50.toFixed(2)} ms | ${budget.coldStartMs} ms | ${coldStatus} |`,
      `| Parse and walk p99 | ${p.p99Us.toFixed(1)} µs | ${budget.parseP99Us} µs | hard: ${status(p.p99Us, budget.parseP99Us)} |`,
      "",
    ].join("\n"),
  );
}
