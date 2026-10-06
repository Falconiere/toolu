import { expect, test } from "bun:test";
import { cpSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { parseUnbash } from "../../../packages/toolu-core/src/shell/__tests__/unbash-parse.ts";
import { z } from "zod";
import { checkUnbashBaseline } from "../check-unbash-baseline.ts";

const ROOT = resolve(import.meta.dir, "../../..");

test("every pinned parser input, including malformed syntax, has a recorded result", () => {
  using sb = createSandbox();
  const shell = join(sb.project, "fixtures/shell");
  mkdirSync(shell, { recursive: true });
  for (const name of [
    "bats-parity.json",
    "issue-283.json",
    "parser-errors.json",
    "unbash-baseline.json",
  ]) {
    cpSync(join(ROOT, "fixtures/shell", name), join(shell, name));
  }
  const pkg = join(sb.project, "packages/toolu-core");
  mkdirSync(pkg, { recursive: true });
  cpSync(join(ROOT, "packages/toolu-core/package.json"), join(pkg, "package.json"));
  expect(checkUnbashBaseline(sb.project)).toBe(203);
  const malformed = z
    .object({ errors: z.array(z.object({ message: z.string() })) })
    .parse(parseUnbash("echo 'unterminated"));
  expect(malformed.errors[0]?.message).toContain("unterminated");

  const baselinePath = join(shell, "unbash-baseline.json");
  const baseline = z
    .looseObject({ cases: z.array(z.object({ input: z.string(), result: z.unknown() })) })
    .parse(JSON.parse(readFileSync(baselinePath, "utf8")));
  writeFileSync(baselinePath, JSON.stringify({ ...baseline, cases: baseline.cases.slice(1) }));
  expect(() => checkUnbashBaseline(sb.project)).toThrow("inputs differ");
  const corrupt = baseline.cases.map((item, index) =>
    index === 0 ? { ...item, result: {} } : item,
  );
  writeFileSync(baselinePath, JSON.stringify({ ...baseline, cases: corrupt }));
  expect(() => checkUnbashBaseline(sb.project)).toThrow("parse differs");
});
