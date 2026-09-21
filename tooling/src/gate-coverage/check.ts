/** Inventory validation, seed metadata, and matrix render. */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
import { discover } from "./discover.ts";
import { fail, rel } from "./fs-util.ts";
import { INVENTORY, MATRIX, ROOT } from "./paths.ts";
import {
  CLASSIFICATIONS,
  type Classification,
  type Discovered,
  type InventoryRow,
  InventoryRowSchema,
  type Support,
} from "./types.ts";

function semanticsNote(d: Discovered): string {
  switch (d.kind) {
    case "hooks.json":
      return `Host routes ${d.event} (${d.matcher || "no matcher"}) to ${d.commandOrModule}.`;
    case "concern":
      return `Assembled quality concern fragment ${d.commandOrModule}; not independently executable.`;
    case "builtin-module":
      return `Built-in dispatcher module ${d.commandOrModule} under ${d.event}.`;
    case "lib":
      return `Shared library sourced by hooks/modules: ${d.commandOrModule}.`;
    case "entrypoint":
      return `Hook entrypoint script ${d.commandOrModule} for ${d.event}.`;
    case "pre-tools.d":
    case "post-tools.d":
    case "session-start.d":
      return `Registry ${d.kind} module ${d.commandOrModule}.`;
    default: {
      throw new Error("unexpected inventory kind: " + d.kind);
    }
  }
}

/** Default classification metadata for a newly discovered row. */
function defaultMeta(d: Discovered): InventoryRow {
  const isLib = d.kind === "lib";
  const isHostOnly =
    d.commandOrModule === "permissions.sh" ||
    (d.plugin === "pr-babysit" && d.commandOrModule === "check-toolu.sh");

  let classification: Classification = "shell-out";
  let support: Support = "required";
  let implementationIssue: number | null = 210;
  let implementationStatus: InventoryRow["implementationStatus"] = "todo";
  let hostMechanism = "pending-opencode";
  let limits = "";

  if (isLib) {
    support = "supported";
    hostMechanism = "native-bash";
  }
  if (isHostOnly) {
    classification = "no-map";
    support = "n/a";
    implementationIssue = null;
    implementationStatus = "n/a";
    hostMechanism = "n/a";
    limits = "Host-specific helper/surface; not an OpenCode enforcement target.";
  }
  if (d.kind === "concern" || d.plugin.endsWith("-quality")) {
    implementationIssue = 204;
  }
  if (d.plugin === "toolu" && (d.kind === "builtin-module" || d.event === "PreToolUse")) {
    implementationIssue = 204;
  }
  if (
    d.kind === "entrypoint" &&
    d.event === "SessionStart" &&
    d.commandOrModule === "session-start.sh"
  ) {
    implementationIssue = 211;
  }

  return {
    ...d,
    semantics: semanticsNote(d),
    classification,
    hostMechanism,
    support,
    implementationIssue,
    implementationStatus,
    verificationBaseline: "none",
    verificationConformance: "pending-#212",
    limits,
    bashRequired: true,
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
**Inventory:** \`tooling/fixtures/gate-coverage/inventory.json\`  
**Check:** \`bun run tooling/src/gate-coverage-inventory.ts check\`

Classifications match [docs/portable-core.md](portable-core.md): \`shell-out\` · \`port-native\` · \`port-new\` · \`no-map\`.

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
    "\n\n## Notes\n\n- Concern fragments are inventoried with `parentId` pointing at `register.sh` when present; they are not independently executable.\n- `hostMechanism=pending-opencode` is resolved against OpenCode pins during #204/#212.\n- Verification conformance links are filled by #212.\n";
  writeFileSync(MATRIX, body);
  process.stdout.write(`gate-coverage-inventory: wrote ${rel(MATRIX)} (${rows.length} rows)\n`);
}

/** Seed inventory JSON + matrix from live discovery (dev helper). */
export function seed(): void {
  const rows = discover().map(defaultMeta);
  mkdirSync(join(ROOT, "tooling/fixtures/gate-coverage"), { recursive: true });
  writeFileSync(INVENTORY, `${JSON.stringify(rows, null, 2)}\n`);
  process.stdout.write(`gate-coverage-inventory: seeded ${rel(INVENTORY)} (${rows.length} rows)\n`);
  render(rows);
}
