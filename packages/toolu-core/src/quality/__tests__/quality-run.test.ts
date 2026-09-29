/**
 * The shared per-file flow and gate settle (#265): real files and real gate
 * files in a sandbox repository, read back through the state layer.
 */
import { expect, test } from "bun:test";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { readGateFile } from "../../state/gate-file.ts";
import { fileQuality, type FileQualitySpec } from "../quality-run.ts";
import { postContext, postEvent, type Call } from "./quality-harness.ts";

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

function runOn(sb: Sandbox, s: FileQualitySpec, call: Call) {
  return fileQuality(postEvent(sb, call), postContext(sb, call), s);
}

function entries(sb: Sandbox): Record<string, { violations: string }> {
  const read = readGateFile(sb.path(GATE));
  if (read.kind !== "ok") return {};
  return read.doc.status === "failing" ? (read.doc.entries ?? {}) : {};
}

test("violations are recorded under the file's path and reported with the standard header", () => {
  using sb = createSandbox({ git: true });
  sb.write("src/a.fx", "x\n");
  const decision = runOn(sb, spec(["first\nexcerpt", "second"]), {
    input: { file_path: "src/a.fx" },
  });
  expect(decision).toEqual({
    kind: "advisory",
    message: "QUALITY VIOLATION — fix before proceeding:\nfirst\nexcerpt\nsecond\n",
  });
  expect(entries(sb)["src/a.fx"]?.violations).toBe("first\nexcerpt\nsecond\n");
});

test("a clean file clears only its own entry and reports the advisories joined by newlines", () => {
  using sb = createSandbox({ git: true });
  sb.write("src/a.fx", "x\n");
  sb.write("src/b.fx", "x\n");
  runOn(sb, spec(["bad a"]), { input: { file_path: "src/a.fx" } });
  runOn(sb, spec(["bad b"]), { input: { file_path: "src/b.fx" } });
  const clean = runOn(sb, spec([], ["", "doc", "", "handler"]), {
    input: { file_path: "src/a.fx" },
  });
  expect(clean).toEqual({ kind: "advisory", message: "doc\nhandler" });
  expect(Object.keys(entries(sb))).toEqual(["src/b.fx"]);
  expect(runOn(sb, spec([]), { input: { file_path: "src/b.fx" } })).toEqual({ kind: "allow" });
  expect(readGateFile(sb.path(GATE))).toMatchObject({ kind: "ok", doc: { status: "passing" } });
});

test("a removed file clears its entry without being checked; a foreign extension is left alone", () => {
  using sb = createSandbox({ git: true });
  sb.write("src/a.fx", "x\n");
  runOn(sb, spec(["bad"]), { input: { file_path: "src/a.fx" } });
  const unrelated: Call = { input: { file_path: "src/a.other", toolu_edit_operation: "delete" } };
  expect(runOn(sb, spec(["never"]), unrelated)).toEqual({ kind: "allow" });
  expect(Object.keys(entries(sb))).toEqual(["src/a.fx"]);
  const deleted: Call = { input: { file_path: "src/a.fx", toolu_edit_operation: "delete" } };
  expect(runOn(sb, spec(["never"]), deleted)).toEqual({ kind: "allow" });
  expect(entries(sb)).toEqual({});
});

test("a missing file, a foreign extension or no path at all is allowed and never recorded", () => {
  using sb = createSandbox({ git: true });
  sb.write("src/a.txt", "x\n");
  for (const call of [
    { input: { file_path: "src/missing.fx" } },
    { input: { file_path: "src/a.txt" } },
    { toolName: "Bash", input: { command: "ls" } },
  ]) {
    expect(runOn(sb, spec(["bad"]), call)).toEqual({ kind: "allow" });
  }
  expect(readGateFile(sb.path(GATE))).toEqual({ kind: "missing" });
});
