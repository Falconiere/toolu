import { expect, test } from "bun:test";
import { join, resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";

// Real-data checks for tooling/src/gate-coverage-inventory.ts (#209).

const ROOT = resolve(import.meta.dir, "../../..");
const CLI = join(ROOT, "tooling/src/gate-coverage-inventory.ts");
const INVENTORY = join(ROOT, "tooling/fixtures/gate-coverage/inventory.json");
const MATRIX = join(ROOT, "docs/gate-coverage-matrix.md");
/** A floor, not a count: future bundled hooks should extend this native inventory. */
const MIN_ROWS = 32;

const Row = z.looseObject({ id: z.string() });
const Rows = z.array(Row);
const HooksRow = z.looseObject({
  id: z.string(),
  kind: z.string(),
  plugin: z.string(),
  event: z.string().optional(),
  commandOrModule: z.string().optional(),
});

async function committedInventory(): Promise<z.infer<typeof Rows>> {
  return Rows.parse(JSON.parse(await Bun.file(INVENTORY).text()));
}

function firstRow(rows: z.infer<typeof Rows>): z.infer<typeof Row> {
  const [row] = rows;
  if (row === undefined) {
    throw new Error("committed inventory is empty");
  }
  return row;
}

function gateCli(env: Record<string, string>): ReturnType<typeof run> {
  return run([process.execPath, "run", CLI, "check"], { cwd: ROOT, env });
}

test.concurrent("discover emits native built-ins and a Bun launcher entry", async () => {
  const res = await run([process.execPath, "run", CLI, "discover"], { cwd: ROOT });
  expect(res.exitCode).toBe(0);
  const ids = Rows.parse(JSON.parse(res.stdout)).map((row) => row.id);
  expect(ids.length).toBeGreaterThanOrEqual(MIN_ROWS);
  expect(ids.some((id) => id.includes("protected-files"))).toBe(true);
  expect(ids).toContain("toolu:builtin-module:PostToolUse:gate-status");
  expect(ids).toContain(
    "ts-quality:hooks.json:SessionStart:register.js:startup|resume|clear|compact",
  );
});

test.concurrent("final inventory and matrix contain only native Bun hooks", async () => {
  const rows = z
    .array(
      z.looseObject({
        id: z.string(),
        sourcePath: z.string(),
        classification: z.string(),
        hostMechanism: z.string(),
        implementationStatus: z.string(),
        bashRequired: z.boolean(),
      }),
    )
    .parse(JSON.parse(await Bun.file(INVENTORY).text()));
  expect(rows.length).toBeGreaterThanOrEqual(MIN_ROWS);
  for (const row of rows) {
    expect(row).toMatchObject({
      classification: "port-native",
      hostMechanism: "bun-bundle",
      implementationStatus: "done",
      bashRequired: false,
    });
    expect(row.sourcePath.endsWith(".sh")).toBe(false);
  }
  const matrix = await Bun.file(MATRIX).text();
  const tableRows = matrix.split("\n").filter((line) => line.startsWith("| `"));
  expect(tableRows).toHaveLength(rows.length);
  expect(tableRows.every((line) => line.includes(" | port-native | "))).toBe(true);
});

test.concurrent("check passes on the committed inventory and matrix", async () => {
  const res = await gateCli({});
  expect(res.exitCode).toBe(0);
  expect((res.stdout + res.stderr).trim()).toBe("gate-coverage-inventory: ok");
});

test.concurrent("check fails when inventory drops a discovered hooks.json id", async () => {
  using sb = createSandbox();
  const rows = z.array(HooksRow).parse(JSON.parse(await Bun.file(INVENTORY).text()));
  const drop = rows.find(
    (row) =>
      row.kind === "hooks.json" &&
      row.plugin === "toolu" &&
      row.event === "PreToolUse" &&
      row.commandOrModule?.endsWith("dist/pre-tools.js") === true,
  )?.id;
  expect(drop).toBeDefined();
  const kept = rows.filter((row) => row.id !== drop);
  expect(kept.length).toBe(rows.length - 1);
  const tmp = sb.write("inventory.json", kept);

  const res = await gateCli({ GATE_COVERAGE_INVENTORY: tmp });
  expect(res.exitCode).not.toBe(0);
});

test.concurrent("check fails when matrix omits an inventory id", async () => {
  using sb = createSandbox();
  const { id } = firstRow(await committedInventory());
  const matrix = await Bun.file(MATRIX).text();
  const kept = matrix.split("\n").filter((line) => !line.includes(id));
  const tmp = sb.write("matrix.md", kept.join("\n"));

  const res = await gateCli({ GATE_COVERAGE_MATRIX: tmp });
  expect(res.exitCode).not.toBe(0);
  expect(res.stdout + res.stderr).toContain(`matrix missing id: ${id}`);
});

test.concurrent("check fails on invalid classification maybe-later", async () => {
  using sb = createSandbox();
  const rows = await committedInventory();
  firstRow(rows).classification = "maybe-later";
  const tmp = sb.write("inventory.json", rows);

  const res = await gateCli({ GATE_COVERAGE_INVENTORY: tmp });
  expect(res.exitCode).not.toBe(0);
  expect(res.stdout + res.stderr).toContain("invalid classification maybe-later");
});

test.concurrent("check fails when support=n/a pairs with shell-out", async () => {
  using sb = createSandbox();
  const rows = await committedInventory();
  const row = firstRow(rows);
  row.classification = "shell-out";
  row.support = "n/a";
  row.implementationIssue = 210;
  const tmp = sb.write("inventory.json", rows);

  const res = await gateCli({ GATE_COVERAGE_INVENTORY: tmp });
  expect(res.exitCode).not.toBe(0);
  expect(res.stdout + res.stderr).toContain("support=n/a requires classification=no-map");
});

test.concurrent("a native built-in module is discovered from its gates source and inventoried port-native", async () => {
  const res = await run([process.execPath, "run", CLI, "discover"], { cwd: ROOT });
  expect(res.exitCode).toBe(0);
  const Discovered = z.array(z.looseObject({ id: z.string(), sourcePath: z.string() }));
  const found = Discovered.parse(JSON.parse(res.stdout)).find(
    (row) => row.id === "toolu:builtin-module:PreToolUse:commit-gate",
  );
  expect(found?.sourcePath).toBe("packages/toolu-core/src/gates/commit-gate.ts");
  const Classified = z.array(z.looseObject({ id: z.string(), classification: z.string() }));
  const rows = Classified.parse(JSON.parse(await Bun.file(INVENTORY).text()));
  const native = ["bash-commands", "commit-gate", "quality-gate"].map((name) =>
    rows.find((row) => row.id === `toolu:builtin-module:PreToolUse:${name}`),
  );
  expect(native.map((row) => row?.classification)).toEqual([
    "port-native",
    "port-native",
    "port-native",
  ]);
  expect(rows.some((row) => row.id.endsWith(":commit-gate.sh"))).toBe(false);
});

test.concurrent("the #262 gates and launchers use native Bun inventory entries", async () => {
  const Classified = z.array(
    z.looseObject({
      id: z.string(),
      classification: z.string(),
      hostMechanism: z.string(),
      bashRequired: z.boolean(),
    }),
  );
  const rows = Classified.parse(JSON.parse(await Bun.file(INVENTORY).text()));
  const ids = [
    "toolu:builtin-module:PreToolUse:push-review",
    "toolu:builtin-module:PreToolUse:plan-ledger",
    "toolu:builtin-module:PreToolUse:docs-sync",
    "toolu:entrypoint:PreToolUse:agent-tier",
  ];
  for (const id of ids) {
    expect(rows.find((row) => row.id === id)).toMatchObject({
      classification: "port-native",
      hostMechanism: "bun-bundle",
      bashRequired: false,
    });
  }
  for (const prefix of [
    "toolu:hooks.json:PreToolUse:pre-tools.js:",
    "toolu:hooks.json:PreToolUse:agent-tier.js:",
  ]) {
    expect(rows.find((row) => row.id.startsWith(prefix))).toMatchObject({
      classification: "port-native",
      hostMechanism: "bun-bundle",
      bashRequired: false,
    });
  }
});

test.concurrent("the #260 native built-ins and the mcp__ bundle entry are port-native", async () => {
  const rows = z
    .array(z.looseObject({ id: z.string(), classification: z.string(), bashRequired: z.boolean() }))
    .parse(JSON.parse(await Bun.file(INVENTORY).text()));
  const ported = rows.filter((row) =>
    [
      "toolu:builtin-module:PreToolUse:code-edit-rules",
      "toolu:builtin-module:PreToolUse:mcp-blocker",
      "toolu:builtin-module:PreToolUse:protected-files",
      "toolu:hooks.json:PreToolUse:mcp-tools.js:mcp__",
    ].includes(row.id),
  );
  expect(ported.map((row) => [row.classification, row.bashRequired])).toEqual([
    ["port-native", false],
    ["port-native", false],
    ["port-native", false],
    ["port-native", false],
  ]);
  const ids = rows.map((row) => row.id);
  expect(
    ids.some((id) => /PreToolUse:(protected-files|mcp-blocker|code-edit-rules)\.sh/.test(id)),
  ).toBe(false);
});
