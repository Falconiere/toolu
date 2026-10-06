/** Verify the committed raw parse results of the pinned unbash version. */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { parseUnbash } from "../../packages/toolu-core/src/shell/__tests__/unbash-parse.ts";
import { z } from "zod";

const SourceSchema = z.looseObject({
  cases: z.array(z.looseObject({ command: z.string().optional() })),
});
const BaselineSchema = z.strictObject({
  version: z.literal(1),
  parser: z.string(),
  cases: z.array(z.strictObject({ input: z.string(), result: z.json() })),
});
const PackageSchema = z.object({ dependencies: z.object({ unbash: z.string() }) });
type Baseline = z.infer<typeof BaselineSchema>;

const SOURCES = ["bats-parity.json", "issue-283.json", "parser-errors.json"];

function readJson(path: string): unknown {
  return JSON.parse(readFileSync(path, "utf8"));
}

/** Compare exact input identity, not just row counts. */
function checkInputs(baseline: Baseline, inputs: readonly string[]): void {
  const actual = baseline.cases.map((item) => item.input).toSorted();
  const expected = [...new Set(inputs)].toSorted();
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error("unbash baseline inputs differ from shell fixture commands");
  }
}

export function checkUnbashBaseline(root: string): number {
  const shell = resolve(root, "fixtures/shell");
  const baseline = BaselineSchema.parse(readJson(resolve(shell, "unbash-baseline.json")));
  const pkg = PackageSchema.parse(readJson(resolve(root, "packages/toolu-core/package.json")));
  if (baseline.parser !== `unbash@${pkg.dependencies.unbash}`) {
    throw new Error("unbash baseline parser version differs from the pinned dependency");
  }
  const inputs = SOURCES.flatMap((source) =>
    SourceSchema.parse(readJson(resolve(shell, source)))
      .cases.map((item) => item.command)
      .filter((command): command is string => command !== undefined),
  );
  checkInputs(baseline, inputs);
  for (const item of baseline.cases) {
    if (JSON.stringify(item.result) !== JSON.stringify(parseUnbash(item.input))) {
      throw new Error(`unbash baseline parse differs for ${JSON.stringify(item.input)}`);
    }
  }
  return baseline.cases.length;
}

if (import.meta.main) {
  const count = checkUnbashBaseline(process.cwd());
  process.stdout.write(`unbash baseline: ${String(count)} distinct inputs match\n`);
}
