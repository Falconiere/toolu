/** Shared per-file flow and gate settlement over JSON inputs and real repos. */
import { expect, test } from "bun:test";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { readGateFile } from "../../state/gate-file.ts";
import { fileQuality, type FileQualitySpec } from "../quality-run.ts";
import { postContext, postEvent } from "./quality-harness.ts";
import { RUN_CASES } from "./runner-cases.ts";

const GATE = ".claude/tmp/quality-gate-status.json";

function spec(errors: string[], advisories: string[] = []): FileQualitySpec {
  return {
    source: "fixture-quality-hook",
    reason: "Post-edit fixture violation(s) detected",
    matches: /\.fx$/,
    skipLinkedWorktrees: true,
    check: () => ({ errors, advisories }),
  };
}

function entries(sb: Sandbox): Record<string, string> {
  const read = readGateFile(sb.path(GATE));
  if (read.kind !== "ok" || read.doc.status !== "failing") return {};
  return Object.fromEntries(
    Object.entries(read.doc.entries ?? {}).map(([path, entry]) => [path, entry.violations]),
  );
}

function gateStatus(sb: Sandbox): string {
  const read = readGateFile(sb.path(GATE));
  if (read.kind === "missing") return "missing";
  if (read.kind === "ok") return read.doc.status;
  return read.kind;
}

for (const c of RUN_CASES) {
  test.concurrent(c.name, () => {
    using sb = createSandbox({ git: true });
    for (const [path, body] of Object.entries(c.files)) sb.write(path, body);
    for (const step of c.steps) {
      const decision = fileQuality(
        postEvent(sb, step.call),
        postContext(sb, step.call),
        spec(step.errors, step.advisories ?? []),
      );
      if (step.decision !== undefined) expect(decision).toEqual(step.decision);
      if (step.entries !== undefined) expect(entries(sb)).toEqual(step.entries);
      if (step.gateStatus !== undefined) expect(gateStatus(sb)).toBe(step.gateStatus);
    }
  });
}
