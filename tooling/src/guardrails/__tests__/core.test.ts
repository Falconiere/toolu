// Ported from the upstream kit's run-fixtures.sh (toolu-conventions@3562c63):
// the whole-tree verdicts, config fail-closed rules, --list and ownership.
import { expect, test } from "bun:test";
import { buildFixture } from "./fixture-tree.ts";
import { CHECK_IDS } from "./golden-cases.ts";
import { count, editConfig, gr } from "./gr-harness.ts";

test.concurrent("AC-1 clean fixture: exit 0, no output", async () => {
  using tree = buildFixture("clean");
  expect(await gr(tree.root, [])).toEqual({ exit: 0, out: "" });
});

test.concurrent("AC-2/AC-3 violating fixture: exit 1, exactly one violation per check, each with a remedy", async () => {
  using tree = buildFixture("violating");
  const { exit, out } = await gr(tree.root, []);
  expect(exit).toBe(1);
  expect(Object.fromEntries(CHECK_IDS.map((id) => [id, count(out, id)]))).toEqual(
    Object.fromEntries(CHECK_IDS.map((id) => [id, 1])),
  );
  const lines = out.split("\n").filter((line) => line.startsWith("guardrails["));
  expect(lines.filter((line) => !line.includes(" — "))).toEqual([]);
});

test.concurrent("AC-7 (TS analogue of missing jq) missing ast-grep: exit 3 naming ast-grep", async () => {
  using tree = buildFixture("violating");
  const res = await gr(tree.root, ["--only", "patterns"], { env: { PATH: "/usr/bin:/bin" } });
  expect(res.exit).toBe(3);
  expect(res.out).toContain("ast-grep not found");
});

test.concurrent("AC-8 missing required key: exit 3 naming the key", async () => {
  using tree = buildFixture("clean");
  editConfig(tree.root, (doc) => {
    delete doc["testDir"];
  });
  const res = await gr(tree.root, []);
  expect(res.exit).toBe(3);
  expect(res.out).toContain("missing required key: testDir");
});

test.concurrent("AC-8 unknown key (typo): exit 3 naming the key", async () => {
  using tree = buildFixture("clean");
  editConfig(tree.root, (doc) => {
    doc["srcRoott"] = "src";
  });
  const res = await gr(tree.root, []);
  expect(res.exit).toBe(3);
  expect(res.out).toContain("unknown key: srcRoott");
});

test.concurrent("AC-8 GR_CONFIG names the config file", async () => {
  using tree = buildFixture("clean");
  const res = await gr(tree.root, [], { env: { GR_CONFIG: "nowhere.json" } });
  expect(res.exit).toBe(3);
  expect(res.out).toContain("no config at nowhere.json");
});

test.concurrent("AC-8 invalid JSON and a wrong-typed key both fail closed", async () => {
  using broken = buildFixture("clean");
  broken.write("guardrails.config.json", "not json\n");
  expect((await gr(broken.root, [])).exit).toBe(3);
  using typed = buildFixture("clean");
  editConfig(typed.root, (doc) => {
    doc["barrelNames"] = "index.ts";
  });
  const res = await gr(typed.root, []);
  expect(res.exit).toBe(3);
  expect(res.out).toContain('"barrelNames" must be an array of strings');
});

test.concurrent("AC-18 a trailing config version warns without failing", async () => {
  using tree = buildFixture("clean");
  editConfig(tree.root, (doc) => {
    doc["version"] = 0;
  });
  const res = await gr(tree.root, []);
  expect(res.exit).toBe(0);
  expect(res.out).toMatch(/warning.*version 0/);
});

test.concurrent("AC-9 --list reports exactly the 14 checks in run order", async () => {
  using tree = buildFixture("clean");
  const res = await gr(tree.root, ["--list"]);
  expect(res.exit).toBe(0);
  expect(res.out.trim().split("\n")).toEqual([...CHECK_IDS]);
});

test.concurrent("unknown flag, empty --only and bare --file exit 3", async () => {
  using tree = buildFixture("clean");
  expect(await gr(tree.root, ["--bogus"])).toEqual({
    exit: 3,
    out: "guardrails: unknown flag: --bogus (try --list)\n",
  });
  expect((await gr(tree.root, ["--only"])).exit).toBe(3);
  expect((await gr(tree.root, ["--file"])).exit).toBe(3);
});

test.concurrent("AC-16/AC-17 required-files and secrets on the violating tree; untracked secret stays silent", async () => {
  using dirty = buildFixture("violating");
  const out = (await gr(dirty.root, [])).out;
  expect(out).toMatch(/wrangler\.jsonc.*required file is missing/);
  expect(out).toMatch(/\.dev\.vars.*tracked by git/);
  using clean = buildFixture("clean");
  clean.write(".dev.vars", "SECRET=local\n");
  expect(count((await gr(clean.root, [])).out, "secrets")).toBe(0);
});

test.concurrent("AC-20 ownedByLinter skips exactly the checks it names; the rest survive", async () => {
  using tree = buildFixture("violating");
  const owned = ["folder-tree", "colocated-tests", "no-barrels", "filename-case", "patterns"];
  editConfig(tree.root, (doc) => {
    doc["ownedByLinter"] = owned;
  });
  const { out } = await gr(tree.root, []);
  expect(owned.map((id) => count(out, id))).toEqual([0, 0, 0, 0, 0]);
  expect([count(out, "folder-readmes"), count(out, "test-tree")]).toEqual([1, 1]);
});

test.concurrent("filename-case alone fails the gate (bash exited 0 here: pipeline subshell)", async () => {
  using tree = buildFixture("violating");
  const res = await gr(tree.root, ["--only", "filename-case"]);
  expect(res.exit).toBe(1);
  expect(count(res.out, "filename-case")).toBe(1);
});
