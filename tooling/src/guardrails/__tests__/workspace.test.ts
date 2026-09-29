// Ported from the upstream kit's run-fixtures.sh workspace block: a real
// two-package git repo driven from its root, the way the hooks and lefthook do.
// packages/api (max 300, owns nothing by linter) and packages/database (max 20,
// folder-tree linter-owned) hold the same 40-line file and the same src/nope/.
import { expect, test } from "bun:test";
import { copyFileSync, mkdirSync, rmSync } from "node:fs";
import { join } from "node:path";
import { buildFixture } from "./fixture-tree.ts";
import type { FixtureTree } from "./fixture-tree.ts";
import { count, editJson, gr } from "./gr-harness.ts";

test.concurrent("AC-1 clean workspace: exit 0, no output", async () => {
  using ws = buildFixture("workspace");
  expect(await gr(ws.root, [])).toEqual({ exit: 0, out: "" });
});

test.concurrent("AC-1/6/7/8 violating workspace: prefixed paths, per-package ceilings and ownership", async () => {
  using ws = buildFixture("workspace-violating");
  const { exit, out } = await gr(ws.root, []);
  expect(exit).toBe(1);
  const violations = out.split("\n").filter((line) => line.startsWith("guardrails["));
  expect(violations.filter((line) => line.includes(" src/"))).toEqual([]);
  expect(out).toContain("packages/database/src/schema/wide-table.ts");
  expect(out).not.toContain("packages/api/src/domains/ping/wide-service.ts");
  expect(out).toContain("packages/api/src/nope");
  expect(out).not.toContain("packages/database/src/nope");
  expect([count(out, "banned-deps"), count(out, "secrets")]).toEqual([1, 2]);
});

test.concurrent("AC-3 --file spanning two packages runs both", async () => {
  using ws = buildFixture("workspace-violating");
  const res = await gr(ws.root, [
    "--file",
    "packages/database/src/schema/wide-table.ts",
    "packages/api/src/nope/README.md",
  ]);
  expect(res.exit).toBe(1);
  expect(res.out).toContain("packages/database/src/schema/wide-table.ts");
  expect(res.out).toContain("packages/api/src/nope");
});

test.concurrent("AC-3 --file on workspace-root files does not block the commit", async () => {
  using ws = buildFixture("workspace");
  expect((await gr(ws.root, ["--file", "package.json", "guardrails.workspace.json"])).exit).toBe(0);
});

function addUnlistedPackage(ws: FixtureTree): void {
  mkdirSync(join(ws.root, "packages/worker/src"), { recursive: true });
  copyFileSync(
    join(ws.root, "packages/api/guardrails.config.json"),
    join(ws.root, "packages/worker/guardrails.config.json"),
  );
}

test.concurrent("AC-3 --file inside an unlisted package still exits 3", async () => {
  using ws = buildFixture("workspace");
  addUnlistedPackage(ws);
  ws.write("packages/worker/src/stray.ts", "export const stray = 1;\n");
  const res = await gr(ws.root, ["--file", "packages/worker/src/stray.ts"]);
  expect(res.exit).toBe(3);
  expect(res.out).toContain('"packages/worker" has a guardrails.config.json but is not listed');
});

const FATAL_SETUPS: ReadonlyArray<readonly [string, (ws: FixtureTree) => void]> = [
  [
    "a package with no guardrails.config.json",
    (ws) => rmSync(join(ws.root, "packages/database/guardrails.config.json")),
  ],
  [
    "a listed package directory that is absent",
    (ws) => rmSync(join(ws.root, "packages/database"), { recursive: true }),
  ],
  [
    "an empty packages array",
    (ws) =>
      editJson(join(ws.root, "guardrails.workspace.json"), (doc) => {
        doc["packages"] = [];
      }),
  ],
  [
    "both a config and a manifest at the root",
    (ws) =>
      copyFileSync(
        join(ws.root, "packages/api/guardrails.config.json"),
        join(ws.root, "guardrails.config.json"),
      ),
  ],
  ["a package that exists but is not listed", addUnlistedPackage],
  [
    "a package path containing ..",
    (ws) =>
      editJson(join(ws.root, "guardrails.workspace.json"), (doc) => {
        doc["packages"] = ["packages/../../elsewhere"];
      }),
  ],
];

for (const [label, setup] of FATAL_SETUPS) {
  test.concurrent(`AC-2 ${label} exits 3`, async () => {
    using ws = buildFixture("workspace");
    setup(ws);
    expect((await gr(ws.root, [])).exit).toBe(3);
  });
}

test.concurrent("AC-2 a misconfigured package outranks a violating one (3 beats 1)", async () => {
  using ws = buildFixture("workspace-violating");
  ws.write("packages/api/guardrails.config.json", "not json at all\n");
  const res = await gr(ws.root, []);
  expect(res.exit).toBe(3);
  // The other package still ran and reported: one broken package does not hide the rest.
  expect(res.out).toContain("packages/database/src/schema/wide-table.ts");
});

test.concurrent("--only is honoured at the root but not forwarded to packages (bash dispatch)", async () => {
  using ws = buildFixture("workspace-violating");
  const { exit, out } = await gr(ws.root, ["--only", "banned-deps"]);
  expect(exit).toBe(1);
  expect(count(out, "banned-deps")).toBe(1);
  expect(count(out, "file-size")).toBe(1);
});

for (const order of ["outer first", "inner first"]) {
  test.concurrent(`a package nested in another is owned by the innermost listing (${order})`, async () => {
    using ws = buildFixture("workspace");
    ws.write("packages/api/inner/src/utilities/README.md", "# utilities\n");
    ws.write("packages/api/inner/wrangler.jsonc", "{}\n");
    copyFileSync(
      join(ws.root, "packages/api/package.json"),
      join(ws.root, "packages/api/inner/package.json"),
    );
    copyFileSync(
      join(ws.root, "packages/api/guardrails.config.json"),
      join(ws.root, "packages/api/inner/guardrails.config.json"),
    );
    const packages = ["packages/api", "packages/api/inner", "packages/database"];
    editJson(join(ws.root, "guardrails.workspace.json"), (doc) => {
      doc["packages"] = order === "outer first" ? packages : [...packages].toReversed();
    });
    const res = await gr(ws.root, []);
    expect(res.out).not.toContain("is not listed");
    expect(res.exit).not.toBe(3);
  });
}
