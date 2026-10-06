/** Content-addressed diff hashes on real repositories from shared JSON cases. */
import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { materializeCaseValue, readCaseFile } from "@toolu/conformance/harness/json-cases";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";
import { diffSha } from "../diff-sha.ts";

const StepSchema = z.discriminatedUnion("op", [
  z.strictObject({ op: z.literal("git"), args: z.array(z.string()) }),
  z.strictObject({ op: z.literal("write"), path: z.string(), body: z.string() }),
  z.strictObject({
    op: z.literal("repeat-write"),
    path: z.string(),
    body: z.string(),
    count: z.number().int().positive(),
  }),
]);
const CaseSchema = z.strictObject({
  name: z.string(),
  kind: z.literal("diff-sha"),
  git: z.boolean(),
  files: z.record(z.string(), z.string()),
  steps: z.array(StepSchema),
  base: z.json(),
  planted: z.json().optional(),
  expect: z.enum(["hash", "nonempty-hash", "empty-blob", "undefined"]),
  hash: z.string().optional(),
  notHash: z.string().optional(),
});
const cases = readCaseFile(resolve(import.meta.dir, "../../../../../fixtures/state/cases.json"))
  .filter((raw) => raw.kind === "diff-sha")
  .map((raw) => CaseSchema.parse(raw));

for (const c of cases) {
  test(c.name, () => {
    using sb = createSandbox({ git: c.git, files: c.files });
    for (const step of c.steps) {
      if (step.op === "git") sb.git(...step.args);
      else if (step.op === "write") sb.write(step.path, step.body);
      else sb.write(step.path, step.body.repeat(step.count));
    }
    const base = z.string().parse(materializeCaseValue(sb, c.base));
    const sha = diffSha(sb.project, base);
    if (c.expect === "undefined") expect(sha).toBeUndefined();
    else if (c.expect === "empty-blob") expect(sha).toBe(c.hash);
    else {
      expect(sha).toMatch(/^[0-9a-f]{40,64}$/);
      if (c.expect === "nonempty-hash") expect(sha).not.toBe(c.notHash);
    }
    if (c.planted !== undefined) {
      const planted = z.string().parse(materializeCaseValue(sb, c.planted));
      expect(existsSync(planted)).toBe(false);
    }
  });
}
