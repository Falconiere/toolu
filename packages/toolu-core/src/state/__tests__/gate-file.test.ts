/** Strict gate-file reads, replacement and clear behavior over shared JSON records. */
import { expect, test } from "bun:test";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { readCaseFile } from "@toolu/conformance/harness/json-cases";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";
import { clearGateFile, readGateFile, recordGateFailure } from "../gate-file.ts";

const CaseSchema = z.strictObject({
  name: z.string(),
  kind: z.literal("gate-file"),
  scenario: z.enum([
    "read",
    "read-reason",
    "replace",
    "clear-unrecognized",
    "clear-malformed",
    "clear-missing",
    "clear-live-lock",
    "telemetry",
    "ordering",
  ]),
  now: z.string(),
  body: z.string().nullable().optional(),
  document: z.json().optional(),
  expectedKind: z.enum(["missing", "malformed", "unrecognized", "ok"]).optional(),
  reasonContains: z.string().optional(),
  branch: z.string().optional(),
  file: z.string().optional(),
  source: z.string().optional(),
  reason: z.string().optional(),
  violations: z.string().optional(),
  expectedDoc: z.json().optional(),
  warningPrefix: z.string().optional(),
  warningSuffix: z.string().optional(),
  warning: z.string().optional(),
  dropLog: z.string().optional(),
  expected: z.union([z.string(), z.array(z.string())]).optional(),
  lockSuffix: z.string().optional(),
  maxMs: z.number().int().optional(),
  telemetryFile: z.string().optional(),
  expectedLine: z.string().optional(),
});
const cases = readCaseFile(resolve(import.meta.dir, "../../../../../fixtures/state/cases.json"))
  .filter((raw) => raw.kind === "gate-file")
  .map((raw) => CaseSchema.parse(raw));
const str = (value: unknown) => z.string().parse(value);

function gate(sb: Sandbox, body?: string): string {
  const dir = join(sb.project, ".claude", "tmp");
  mkdirSync(dir, { recursive: true });
  const file = join(dir, "quality-gate-status.json");
  if (body !== undefined) writeFileSync(file, body);
  return file;
}

for (const c of cases) {
  test(c.name, () => {
    using sb = createSandbox({
      git: c.branch !== undefined,
      ...(c.branch === undefined ? {} : { branch: c.branch }),
    });
    const body =
      c.document === undefined ? (c.body ?? undefined) : JSON.stringify(c.document, null, 2);
    const file = gate(sb, body);
    const warnings: string[] = [];
    const options = {
      env: { HOME: sb.home, TOOLU_PROJECT_DIR: sb.project, TOOLU_HOST_OVERRIDE: "claude" },
      host: "claude" as const,
      now: () => new Date(c.now),
      warn: (m: string) => warnings.push(m),
    };
    if (c.scenario === "read") {
      expect(readGateFile(file).kind).toBe(
        z.enum(["missing", "malformed", "unrecognized", "ok"]).parse(c.expectedKind),
      );
    } else if (c.scenario === "read-reason") {
      const read = readGateFile(file);
      expect(read.kind === "unrecognized" && read.reason).toContain(str(c.reasonContains));
    } else if (c.scenario === "replace") {
      recordGateFailure(
        file,
        str(c.file),
        str(c.source),
        str(c.reason),
        str(c.violations),
        options,
      );
      const read = readGateFile(file);
      expect(read.kind).toBe("ok");
      expect<unknown>(read.kind === "ok" && read.doc).toEqual(c.expectedDoc);
      expect(warnings).toHaveLength(1);
      expect(warnings[0]).toStartWith(str(c.warningPrefix).replace("$GATE", file));
      expect(readFileSync(`${file}.dropped.log`, "utf8")).toBe(str(c.dropLog));
    } else if (c.scenario === "clear-unrecognized") {
      expect<unknown>(clearGateFile(file, str(c.file), str(c.source), options)).toBe(
        str(c.expected),
      );
      expect(readFileSync(file, "utf8")).toBe(str(body));
      expect(warnings).toHaveLength(1);
      expect(warnings[0]).toEndWith(str(c.warningSuffix));
      expect(existsSync(join(sb.project, ".claude", "tmp", "telemetry"))).toBe(false);
    } else if (c.scenario === "clear-malformed") {
      expect<unknown>(clearGateFile(file, str(c.file), str(c.source), options)).toBe(
        str(c.expected),
      );
      expect(warnings).toEqual([str(c.warning).replace("$GATE", file)]);
      expect(readFileSync(file, "utf8")).toBe(str(c.body));
    } else if (c.scenario === "clear-missing") {
      expect<unknown>(clearGateFile(file, str(c.file), str(c.source), options)).toBe(
        str(c.expected),
      );
      expect(warnings).toEqual([]);
    } else if (c.scenario === "clear-live-lock") {
      const lock = `${String(process.pid)}${str(c.lockSuffix)}`;
      writeFileSync(`${file}.lock`, lock);
      const started = Date.now();
      expect<unknown>(clearGateFile(file, str(c.file), str(c.source), options)).toBe(
        str(c.expected),
      );
      expect(Date.now() - started).toBeLessThan(z.number().parse(c.maxMs));
      expect(warnings).toEqual([]);
      expect(readFileSync(`${file}.lock`, "utf8")).toBe(lock);
    } else if (c.scenario === "telemetry") {
      recordGateFailure(
        file,
        str(c.file),
        str(c.source),
        str(c.reason),
        str(c.violations),
        options,
      );
      expect(
        readFileSync(join(sb.project, ".claude", "tmp", "telemetry", str(c.telemetryFile)), "utf8"),
      ).toBe(str(c.expectedLine));
    } else {
      recordGateFailure(
        file,
        str(c.file),
        str(c.source),
        str(c.reason),
        str(c.violations),
        options,
      );
      const read = readGateFile(file);
      expect(
        read.kind === "ok" && read.doc.status === "failing" && Object.keys(read.doc.entries ?? {}),
      ).toEqual(z.array(z.string()).parse(c.expected));
    }
  });
}
