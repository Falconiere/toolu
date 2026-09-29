// Ported from the upstream kit's run-fixtures.sh: --file mode and the
// file-addressable checks (folder-tree, file-size, filename-case, patterns,
// secret-content, no-barrels), each against a real fixture repo.
import { expect, test } from "bun:test";
import { chmodSync, cpSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { buildFixture } from "./fixture-tree.ts";
import { FAKE_AWS_KEY, count, editConfig, gr, merge } from "./gr-harness.ts";

const SET_LEVEL = [
  "folder-readmes",
  "test-tree",
  "banned-deps",
  "shadow-configs",
  "required-files",
  "secrets",
];

function lines(n: number, make: (i: number) => string): string {
  return [...Array(n).keys()].map((i) => `${make(i + 1)}\n`).join("");
}

test.concurrent("AC-4 --file reports folder-tree; a clean file is silent; set-level checks never run", async () => {
  using tree = buildFixture("violating");
  const helper = await gr(tree.root, ["--file", "src/domains/shifts/utils/helper.ts"]);
  expect(helper.exit).toBe(1);
  expect(count(helper.out, "folder-tree")).toBeGreaterThanOrEqual(1);
  const plain = await gr(tree.root, ["--file", "src/utilities/plain.ts"]);
  expect(plain).toEqual({ exit: 0, out: "" });
  expect(SET_LEVEL.map((id) => count(helper.out + plain.out, id))).toEqual([0, 0, 0, 0, 0, 0]);
});

test.concurrent("AC-4 --file accepts several paths and reports each", async () => {
  using tree = buildFixture("violating");
  const res = await gr(tree.root, [
    "--file",
    "src/ui/index.ts",
    "src/domains/shifts/utils/helper.ts",
  ]);
  expect(res.exit).toBe(1);
  expect(count(res.out, "no-barrels")).toBe(1);
  expect(count(res.out, "folder-tree")).toBeGreaterThanOrEqual(1);
});

test.concurrent("AC-4 --file on files no rule can parse: silent exit 0", async () => {
  using tree = buildFixture("clean");
  tree.write("src/utilities/notes.md", "# notes\n");
  tree.write("src/utilities/data.yaml", "key: value\n");
  expect(
    await gr(tree.root, ["--file", "src/utilities/notes.md", "src/utilities/data.yaml"]),
  ).toEqual({
    exit: 0,
    out: "",
  });
});

test.concurrent("AC-4 filename-case in --file mode, silent on a kebab-case name, and a spaced path", async () => {
  using tree = buildFixture("violating");
  const bad = await gr(tree.root, ["--file", "src/utilities/BadName.ts"]);
  expect([bad.exit, count(bad.out, "filename-case")]).toEqual([1, 1]);
  expect(
    count((await gr(tree.root, ["--file", "src/utilities/plain.ts"])).out, "filename-case"),
  ).toBe(0);
  using clean = buildFixture("clean");
  clean.write("src/utilities/Bad Name.ts", "export const spaced = 1;\n");
  const spaced = await gr(clean.root, ["--file", "src/utilities/Bad Name.ts"]);
  expect([spaced.exit, count(spaced.out, "filename-case")]).toEqual([1, 1]);
});

test.concurrent("AC-9 folder-tree rejects an intra-domain dir; a domain without README fails", async () => {
  using dirty = buildFixture("violating");
  expect((await gr(dirty.root, [])).out).toMatch(/utils.*not an allowed directory/);
  using clean = buildFixture("clean");
  rmSync(join(clean.root, "src/domains/shifts/README.md"));
  expect((await gr(clean.root, [])).out).toContain("domain folder has no README");
});

test.concurrent("AC-10 blank and comment lines never count; 301 code lines fail at 300", async () => {
  using tree = buildFixture("clean");
  editConfig(tree.root, (doc) => merge(doc, "fileSize", { max: 300 }));
  tree.write(
    "src/utilities/mostly-docs.ts",
    lines(250, (i) => `export const v${String(i)} = ${String(i)};`) +
      lines(150, (i) => `\n// filler ${String(i)}`),
  );
  expect(count((await gr(tree.root, [])).out, "file-size")).toBe(0);
  tree.write(
    "src/utilities/too-big.ts",
    lines(301, (i) => `export const w${String(i)} = ${String(i)};`),
  );
  expect((await gr(tree.root, [])).out).toContain("301 code lines");
});

test.concurrent("AC-10 crate-root and nested tests/ files are exempt from the size ceiling", async () => {
  using tree = buildFixture("clean");
  editConfig(tree.root, (doc) => {
    doc["testDir"] = "tests";
    merge(doc, "fileSize", { max: 20, skipExtensions: [] });
  });
  const body = lines(40, (i) => `line ${String(i)};`);
  tree.write("tests/cli.rs", body);
  tree.write("src/mod/tests/unit.rs", body);
  expect(count((await gr(tree.root, ["--file", "tests/cli.rs"])).out, "file-size")).toBe(0);
  expect(count((await gr(tree.root, ["--file", "src/mod/tests/unit.rs"])).out, "file-size")).toBe(
    0,
  );
});

test.concurrent("AC-11 patterns: no-direct-env-var fires on rust source, silent on the clean tree", async () => {
  using dirty = buildFixture("violating");
  expect((await gr(dirty.root, [])).out).toContain("no-direct-env-var");
  using clean = buildFixture("clean");
  expect(count((await gr(clean.root, [])).out, "patterns")).toBe(0);
});

test.concurrent("AC-19 a malformed ast-grep rule exits 3, never a silent green", async () => {
  // A private copy of the runner and its rules, so the broken rule never touches the repo.
  using sb = createSandbox();
  const tooling = resolve(import.meta.dir, "../../..");
  cpSync(join(tooling, "src/guardrails"), sb.path("tooling/src/guardrails"), { recursive: true });
  cpSync(
    join(tooling, "conventions/guardrails/patterns"),
    sb.path("tooling/conventions/guardrails/patterns"),
    {
      recursive: true,
    },
  );
  sb.write(
    "tooling/conventions/guardrails/patterns/rust/zz-broken.yml",
    'id: zz-broken\nlanguage: rust\nrule:\n  pattern: "((("\n',
  );
  using tree = buildFixture("violating");
  const res = await gr(tree.root, ["--only", "patterns"], {
    runner: sb.path("tooling/src/guardrails/run.ts"),
  });
  expect(res.exit).toBe(3);
  expect(res.out).toContain("ast-grep exited");
});

test.concurrent("AC-21 secret-content: repo and --file modes, silent on clean files", async () => {
  using dirty = buildFixture("violating");
  expect((await gr(dirty.root, [])).out).toMatch(
    /leaked-key\.ts.*committed secret value \(AWS access key id\)/,
  );
  const file = await gr(dirty.root, ["--file", "src/utilities/leaked-key.ts"]);
  expect([file.exit, count(file.out, "secret-content")]).toEqual([1, 1]);
  expect(
    count((await gr(dirty.root, ["--file", "src/utilities/plain.ts"])).out, "secret-content"),
  ).toBe(0);
  using clean = buildFixture("clean");
  expect(count((await gr(clean.root, [])).out, "secret-content")).toBe(0);
});

test.concurrent("AC-21 dash-prefixed and non-ASCII tracked names neither drain nor bypass the scan", async () => {
  using tree = buildFixture("clean", {
    "src/utilities/-decoy.ts": "export const decoy = 1;\n",
    "src/utilities/café.ts": `export const leaked = '${FAKE_AWS_KEY}';\n`,
    "src/utilities/zzz-clean.ts": "export const clean2 = 1;\n",
  });
  const { out } = await gr(tree.root, ["--only", "secret-content"]);
  expect(count(out, "secret-content")).toBe(1);
  expect(out).toMatch(/café\.ts.*committed secret value \(AWS access key id\)/);
});

test.concurrent("AC-21 a colon in a tracked filename keeps the full path in the report", async () => {
  using tree = buildFixture("clean", {
    "src/utilities/weird:name.ts": `export const leaked = '${FAKE_AWS_KEY}';\n`,
  });
  const { out } = await gr(tree.root, ["--only", "secret-content"]);
  expect(count(out, "secret-content")).toBe(1);
  expect(out).toMatch(/weird:name\.ts.*committed secret value \(AWS access key id\)/);
});

test.concurrent("secret-content skips binary files and honours secrets.scanExempt", async () => {
  using tree = buildFixture("clean", {
    "src/utilities/blob.bin": `\0${FAKE_AWS_KEY}\n`,
    "src/utilities/exempt-key.ts": `export const k = '${FAKE_AWS_KEY}';\n`,
  });
  editConfig(tree.root, (doc) => merge(doc, "secrets", { scanExempt: ["src/utilities/exempt-*"] }));
  expect(count((await gr(tree.root, ["--only", "secret-content"])).out, "secret-content")).toBe(0);
});

test.concurrent("AC-22 barrelNames=[mod.rs] fires with the Rust-specific remedy", async () => {
  using tree = buildFixture("clean");
  editConfig(tree.root, (doc) => {
    doc["barrelNames"] = ["mod.rs"];
  });
  mkdirSync(join(tree.root, "src/store"), { recursive: true });
  writeFileSync(join(tree.root, "src/store/mod.rs"), "pub mod shift_store;\n");
  const { out } = await gr(tree.root, ["--file", "src/store/mod.rs"]);
  expect(count(out, "no-barrels")).toBe(1);
  expect(out).toContain("sibling file");
});

test.concurrent("AC-19 an ast-grep killed by a signal exits 3, never a silent green", async () => {
  // The package-local binary is resolved first, the way a devDependency would be.
  using tree = buildFixture("violating", {
    "node_modules/.bin/ast-grep": "#!/bin/sh\nkill -9 $$\n",
  });
  chmodSync(join(tree.root, "node_modules/.bin/ast-grep"), 0o755);
  const res = await gr(tree.root, ["--only", "patterns"]);
  expect(res.exit).toBe(3);
  expect(res.out).toContain("ast-grep was killed by SIGKILL");
});
