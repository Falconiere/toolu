/** Verify that shared JSON case suites preserve their recorded case names. */
import { readFileSync } from "node:fs";
import { isAbsolute, relative, resolve } from "node:path";
import { readCaseFile } from "@toolu/conformance/harness/json-cases";
import { z } from "zod";

const SuiteSchema = z.strictObject({
  id: z.string().min(1),
  fixture: z.string().min(1),
  source: z.string().min(1),
  namePrefix: z.string().min(1).optional(),
  names: z.array(z.string().min(1)).min(1),
});
const IndexSchema = z.strictObject({ version: z.literal(1), suites: z.array(SuiteSchema).min(1) });

type FixtureSuite = z.infer<typeof SuiteSchema>;
type FixtureIndex = z.infer<typeof IndexSchema>;

function unique(items: readonly string[], label: string): void {
  if (new Set(items).size !== items.length) throw new Error(`${label}: duplicate value`);
}

export function readFixtureIndex(path: string): FixtureIndex {
  const index = IndexSchema.parse(JSON.parse(readFileSync(path, "utf8")));
  unique(
    index.suites.map((suite) => suite.id),
    "fixture suite IDs",
  );
  for (const suite of index.suites) unique(suite.names, `${suite.id} names`);
  return index;
}

export function checkSuite(root: string, suite: FixtureSuite): void {
  const file = resolve(root, suite.fixture);
  const back = relative(root, file);
  if (back.startsWith("..") || isAbsolute(back)) {
    throw new Error(`${suite.id}: fixture path escapes root`);
  }
  const actual = readCaseFile(file)
    .filter((item) => suite.namePrefix === undefined || item.name.startsWith(suite.namePrefix))
    .map((item) => item.name)
    .toSorted();
  const expected = suite.names.toSorted();
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`${suite.id}: case names differ from recorded baseline`);
  }
}

export function checkInventory(root: string, index: FixtureIndex): void {
  const byFile = new Map<string, FixtureSuite[]>();
  for (const suite of index.suites) {
    checkSuite(root, suite);
    byFile.set(suite.fixture, [...(byFile.get(suite.fixture) ?? []), suite]);
  }
  for (const [file, suites] of byFile) {
    const names = readCaseFile(resolve(root, file))
      .map((item) => item.name)
      .toSorted();
    const covered = suites.flatMap((suite) => suite.names).toSorted();
    if (JSON.stringify(names) !== JSON.stringify(covered)) {
      throw new Error(`${file}: suites do not cover every case exactly once`);
    }
  }
}

if (import.meta.main) {
  const root = process.cwd();
  const index = readFixtureIndex(resolve(root, "fixtures/index.json"));
  checkInventory(root, index);
  process.stdout.write(`fixture inventory: ${String(index.suites.length)} suites match baseline\n`);
}
