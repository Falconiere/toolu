/** Inventory validation, seed metadata, and matrix render. */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
import { discover } from "./discover.ts";
import { fail, rel } from "./fs-util.ts";
import { INVENTORY, MATRIX, ROOT } from "./paths.ts";
import {
  CLASSIFICATIONS,
  type Discovered,
  type InventoryRow,
  InventoryRowSchema,
} from "./types.ts";

function semanticsNote(d: Discovered): string {
  switch (d.kind) {
    case "hooks.json":
      return `Host routes ${d.event} (${d.matcher || "no matcher"}) to ${d.commandOrModule}.`;
    case "builtin-module":
      return `Built-in dispatcher module ${d.commandOrModule} under ${d.event}.`;
    case "entrypoint":
      return `Hook entrypoint ${d.commandOrModule} for ${d.event}.`;
    default: {
      throw new Error("unexpected inventory kind: " + d.kind);
    }
  }
}

/** Default classification metadata for a newly discovered row. */
function defaultMeta(d: Discovered): InventoryRow {
  return {
    ...d,
    semantics: semanticsNote(d),
    classification: "port-native",
    support: "required",
    implementationIssue: d.kind === "builtin-module" && d.event === "PostToolUse" ? 259 : 279,
    implementationStatus: "done",
    verificationBaseline: "none",
    verificationConformance: "bun run test:conformance",
    limits: "",
    bashRequired: false,
  };
}

function validateRow(row: InventoryRow, errors: string[]): void {
  if (!row.id) errors.push("row missing id");
  if (row.id.includes('"') || row.id.includes("'")) {
    errors.push(`${row.id}: id must not contain quote characters`);
  }
  if (!CLASSIFICATIONS.has(row.classification)) {
    errors.push(`${row.id}: invalid classification ${row.classification}`);
  }
  if (row.classification !== "port-native") {
    errors.push(`${row.id}: final inventory requires classification=port-native`);
  }
  if (row.bashRequired) errors.push(`${row.id}: final inventory requires bashRequired=false`);
  if (row.implementationStatus !== "done") {
    errors.push(`${row.id}: final inventory requires implementationStatus=done`);
  }
  if (row.classification === "no-map" && !row.limits.trim()) {
    errors.push(`${row.id}: no-map requires limits rationale`);
  }
  if (row.support === "n/a" && row.classification !== "no-map") {
    errors.push(`${row.id}: support=n/a requires classification=no-map`);
  }
  if (row.support === "required" && row.implementationIssue == null) {
    errors.push(`${row.id}: required row needs implementationIssue`);
  }
  if (row.commandOrModule.includes('"') || row.commandOrModule.includes("'")) {
    errors.push(`${row.id}: commandOrModule must not contain quote characters`);
  }
}

/** Load the committed inventory JSON (row fields validated in check()). */
export function loadInventory(): InventoryRow[] {
  if (!existsSync(INVENTORY)) fail(`missing inventory ${INVENTORY}`);
  const raw: unknown = JSON.parse(readFileSync(INVENTORY, "utf8"));
  const parsed = z.array(InventoryRowSchema).safeParse(raw);
  if (!parsed.success) fail(`inventory schema invalid: ${parsed.error.message}`);
  return parsed.data;
}

/** Fail closed when discovery, inventory, or matrix drift. */
export function check(): void {
  const discovered = discover();
  const inventory = loadInventory();
  const errors: string[] = [];
  const discIds = new Set(discovered.map((d) => d.id));
  const invIds = new Set(inventory.map((r) => r.id));
  for (const id of discIds) {
    if (!invIds.has(id)) errors.push(`missing from inventory: ${id}`);
  }
  for (const id of invIds) {
    if (!discIds.has(id)) errors.push(`orphan inventory id: ${id}`);
  }
  const discoveredById = new Map(discovered.map((row) => [row.id, row]));
  for (const row of inventory) {
    const live = discoveredById.get(row.id);
    if (!live) continue;
    for (const key of [
      "sourcePath",
      "plugin",
      "kind",
      "event",
      "matcher",
      "commandOrModule",
      "hostMechanism",
      "parentId",
    ] as const) {
      if (row[key] !== live[key]) errors.push(`${row.id}: ${key} differs from discovery`);
    }
  }
  for (const row of inventory) validateRow(row, errors);
  if (!existsSync(MATRIX)) fail(`missing matrix ${MATRIX}`);
  const matrix = readFileSync(MATRIX, "utf8");
  for (const id of invIds) {
    if (!matrix.includes(id)) errors.push(`matrix missing id: ${id}`);
  }
  if (errors.length) {
    for (const e of errors) console.error(`gate-coverage-inventory: ${e}`);
    process.exit(1);
  }
  process.stdout.write("gate-coverage-inventory: ok\n");
}

/** Rewrite docs/gate-coverage-matrix.md from inventory rows. */
export function render(rows: InventoryRow[]): void {
  const header = `# Gate coverage matrix

**Issue:** [#209](https://github.com/Falconiere/toolu/issues/209) (epic [#203](https://github.com/Falconiere/toolu/issues/203))  
**Final removal:** [#279](https://github.com/Falconiere/toolu/issues/279) (epic [#247](https://github.com/Falconiere/toolu/issues/247))
**Inventory:** \`fixtures/gate-coverage/inventory.json\`

**Check:** \`bun run tooling/src/gate-coverage-inventory.ts check\`

Every live hook and built-in gate in this inventory is \`port-native\`. The host mechanism records whether its entry uses a Bun bundle or the generated native launcher. The final-removal check also rejects tracked shell and Bats files, except the root curl installer \`install.sh\` (#457) and the one-line native shims \`plugins/jev/scripts/jev.sh\` and \`plugins/toolu-review/scripts/write-state.sh\` (#440).

| id | source | plugin | event | classification | support | impl | host mechanism | bash | limits |
|----|--------|--------|-------|----------------|---------|------|----------------|------|--------|
`;
  const lines = rows
    .toSorted((a, b) => a.id.localeCompare(b.id))
    .map((r) => {
      const impl =
        r.implementationIssue == null
          ? r.implementationStatus
          : `#${r.implementationIssue}/${r.implementationStatus}`;
      return `| \`${r.id}\` | \`${r.sourcePath}\` | ${r.plugin} | ${r.event} | ${r.classification} | ${r.support} | ${impl} | ${r.hostMechanism} | ${r.bashRequired ? "yes" : "no"} | ${r.limits || "—"} |`;
    });
  const body =
    header +
    lines.join("\n") +
    "\n\n## Notes\n\n- Native built-in gate rows point to their TypeScript source and the dispatcher hook that invokes them.\n- OpenCode supports the host events documented in [conformance-report.md](conformance-report.md); this inventory records the implementation of each listed hook rather than claiming every host exposes every event.\n";
  writeFileSync(MATRIX, body);
  process.stdout.write(`gate-coverage-inventory: wrote ${rel(MATRIX)} (${rows.length} rows)\n`);
}

/** Seed inventory JSON + matrix from live discovery (dev helper). */
export function seed(): void {
  const PreviousRow = z.object({
    id: z.string(),
    semantics: z.string(),
    implementationIssue: z.number().nullable(),
    implementationStatus: z.string(),
    verificationBaseline: z.string(),
    verificationConformance: z.string(),
  });
  const previous = existsSync(INVENTORY)
    ? new Map(
        z
          .array(PreviousRow)
          .parse(JSON.parse(readFileSync(INVENTORY, "utf8")))
          .map((row) => [row.id, row]),
      )
    : new Map<string, z.infer<typeof PreviousRow>>();
  const rows = discover().map((d) => {
    const old = previous.get(d.id);
    const row = defaultMeta(d);
    if (!old) return row;
    const citedIssue = [...old.semantics.matchAll(/#(\d+)/gu)].at(-1)?.[1];
    return Object.assign(row, {
      semantics: /bash fallback|shell-out|legacy/i.test(old.semantics)
        ? row.semantics
        : old.semantics,
      implementationIssue:
        old.implementationStatus === "done" && old.implementationIssue !== 279
          ? old.implementationIssue
          : citedIssue === undefined
            ? row.implementationIssue
            : Number(citedIssue),
      verificationBaseline: old.verificationBaseline,
      verificationConformance:
        old.verificationConformance === "pending-#212"
          ? "bun run test:conformance"
          : old.verificationConformance,
    });
  });
  mkdirSync(join(ROOT, "fixtures/gate-coverage"), { recursive: true });
  writeFileSync(INVENTORY, `${JSON.stringify(rows, null, 2)}\n`);
  process.stdout.write(`gate-coverage-inventory: seeded ${rel(INVENTORY)} (${rows.length} rows)\n`);
  render(rows);
}
