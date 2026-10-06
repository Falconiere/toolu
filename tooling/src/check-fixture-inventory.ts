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

type CapturePair = {
  fixture: string;
  golden: string;
  field: string;
  hostSuffix?: boolean;
};

const CAPTURE_PAIRS: CapturePair[] = [
  ...["lifecycle", "pre-tool-modules-a", "pre-tool-modules-b", "pre-tool-modules-c"].map(
    (name) => ({
      fixture: `gates/${name}.json`,
      golden: `gates/${name}-golden.json`,
      field: "cases",
    }),
  ),
  ...["ts", "python", "rust"].map((name) => ({
    fixture: `quality/${name}.json`,
    golden: `quality/${name}-golden.json`,
    field: "cases",
    hostSuffix: true,
  })),
  ...["nudge", "savings", "report"].map((name) => ({
    fixture: `ast-grep/${name}.json`,
    golden: "ast-grep/golden.json",
    field: name,
    hostSuffix: name !== "report",
  })),
];

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

export function checkCaptureFile(root: string, pair: CapturePair): void {
  const names = new Set(
    readCaseFile(resolve(root, "fixtures", pair.fixture)).map((item) => item.name),
  );
  const golden = z
    .record(z.string(), z.unknown())
    .parse(JSON.parse(readFileSync(resolve(root, "fixtures", pair.golden), "utf8")));
  const captures = z.record(z.string(), z.unknown()).parse(golden[pair.field]);
  const covered = new Set<string>();
  for (const key of Object.keys(captures)) {
    const name = pair.hostSuffix ? key.replace(/ \[(claude|codex)\]$/, "") : key;
    if (!names.has(name) || (pair.hostSuffix && name === key)) {
      throw new Error(`${pair.golden}:${pair.field}: orphaned capture ${key}`);
    }
    covered.add(name);
  }
  for (const name of names) {
    if (!covered.has(name))
      throw new Error(`${pair.golden}:${pair.field}: missing capture ${name}`);
  }
}

export function checkCommittedCaptures(root: string): void {
  for (const pair of CAPTURE_PAIRS) checkCaptureFile(root, pair);
}

if (import.meta.main) {
  const root = process.cwd();
  const index = readFixtureIndex(resolve(root, "fixtures/index.json"));
  checkInventory(root, index);
  checkCommittedCaptures(root);
  process.stdout.write(`fixture inventory: ${String(index.suites.length)} suites match baseline\n`);
}
