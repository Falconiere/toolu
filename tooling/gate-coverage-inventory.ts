#!/usr/bin/env bun
/**
 * Gate coverage inventory for #209 — discover repo hook/behavior sources,
 * check committed inventory + matrix drift, render the human matrix.
 *
 * Usage:
 *   bun run tooling/gate-coverage-inventory.ts discover
 *   bun run tooling/gate-coverage-inventory.ts check
 *   bun run tooling/gate-coverage-inventory.ts render
 *   bun run tooling/gate-coverage-inventory.ts seed
 */
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, join, relative } from "node:path";

const ROOT = join(import.meta.dir, "..");
const INVENTORY =
  process.env.GATE_COVERAGE_INVENTORY ??
  join(ROOT, "tooling/fixtures/gate-coverage/inventory.json");
const MATRIX =
  process.env.GATE_COVERAGE_MATRIX ?? join(ROOT, "docs/gate-coverage-matrix.md");

const CLASSIFICATIONS = new Set<string>(["shell-out", "port-native", "port-new", "no-map"]);
type Classification = "shell-out" | "port-native" | "port-new" | "no-map";
type Support = "required" | "supported" | "blocked" | "n/a";
type Kind =
  | "hooks.json"
  | "concern"
  | "pre-tools.d"
  | "post-tools.d"
  | "session-start.d"
  | "builtin-module"
  | "entrypoint"
  | "lib";

type InventoryRow = {
  id: string;
  sourcePath: string;
  plugin: string;
  kind: Kind;
  event: string;
  matcher: string;
  commandOrModule: string;
  parentId: string | null;
  semantics: string;
  classification: Classification;
  hostMechanism: string;
  support: Support;
  implementationIssue: number | null;
  implementationStatus: "todo" | "wip" | "done" | "n/a";
  verificationBaseline: string;
  verificationConformance: string;
  limits: string;
  bashRequired: boolean;
};

type Discovered = {
  id: string;
  sourcePath: string;
  plugin: string;
  kind: Kind;
  event: string;
  matcher: string;
  commandOrModule: string;
  parentId: string | null;
};

function fail(msg: string): never {
  console.error(`gate-coverage-inventory: ${msg}`);
  process.exit(1);
}

function rel(p: string): string {
  return relative(ROOT, p).split("\\").join("/");
}

function listDirNames(dir: string): string[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true })
    .filter((e) => e.isDirectory())
    .map((e) => e.name)
    .sort();
}

function listShFiles(dir: string, pattern?: RegExp): string[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true })
    .filter((e) => e.isFile() && e.name.endsWith(".sh") && (!pattern || pattern.test(e.name)))
    .map((e) => join(dir, e.name))
    .sort();
}

function normalizeCommand(command: string): string {
  let c = command.trim();
  // hooks.json sometimes embeds quotes inside the JSON string value (e.g. jev).
  if (
    (c.startsWith('"') && c.endsWith('"')) ||
    (c.startsWith("'") && c.endsWith("'"))
  ) {
    c = c.slice(1, -1);
  }
  return c.replace("${CLAUDE_PLUGIN_ROOT}/", "").replace("${PLUGIN_ROOT}/", "");
}

function sanitizeIdPart(s: string): string {
  return s.replace(/["']/g, "").replace(/\s+/g, "_");
}

function makeId(plugin: string, kind: Kind, event: string, commandOrModule: string, matcher = ""): string {
  const base = sanitizeIdPart(basename(commandOrModule));
  const m = matcher ? `:${sanitizeIdPart(matcher).slice(0, 40)}` : "";
  return `${plugin}:${kind}:${event}:${base}${m}`;
}

function discover(): Discovered[] {
  const out: Discovered[] = [];
  const seen = new Set<string>();

  const add = (row: Discovered) => {
    let id = row.id;
    if (seen.has(id)) id = `${id}#${seen.size}`;
    seen.add(id);
    out.push({ ...row, id });
  };

  for (const plugin of listDirNames(join(ROOT, "plugins"))) {
    const hooksJson = join(ROOT, "plugins", plugin, "hooks", "hooks.json");
    if (existsSync(hooksJson)) {
      const raw = JSON.parse(readFileSync(hooksJson, "utf8")) as {
        hooks?: Record<string, unknown[]>;
      };
      const hooks = raw.hooks ?? {};
      for (const [event, entries] of Object.entries(hooks)) {
        if (!Array.isArray(entries)) continue;
        for (const entry of entries) {
          const e = entry as { matcher?: string; hooks?: { command?: string }[]; command?: string };
          const matcher = e.matcher ?? "";
          const commands: string[] = [];
          if (Array.isArray(e.hooks)) {
            for (const h of e.hooks) {
              if (h.command) commands.push(h.command);
            }
          } else if (e.command) {
            commands.push(e.command);
          }
          for (const command of commands) {
            const commandOrModule = normalizeCommand(command);
            add({
              id: makeId(plugin, "hooks.json", event, commandOrModule, matcher),
              sourcePath: rel(hooksJson),
              plugin,
              kind: "hooks.json",
              event,
              matcher,
              commandOrModule,
              parentId: null,
            });
          }
        }
      }
    }

    const concernsDir = join(ROOT, "plugins", plugin, "hooks", "concerns");
    const parentId = `${plugin}:entrypoint:SessionStart:register.sh`;
    for (const abs of listShFiles(concernsDir, /^[0-9]{2}-.*\.sh$/)) {
      const name = basename(abs);
      add({
        id: makeId(plugin, "concern", "PostToolUse", name),
        sourcePath: rel(abs),
        plugin,
        kind: "concern",
        event: "PostToolUse",
        matcher: "",
        commandOrModule: name,
        parentId: existsSync(join(ROOT, "plugins", plugin, "hooks", "register.sh")) ? parentId : null,
      });
    }

    for (const dname of ["pre-tools.d", "post-tools.d", "session-start.d"] as const) {
      const d = join(ROOT, "plugins", plugin, "hooks", dname);
      const kind = dname as Kind;
      const event =
        dname === "pre-tools.d" ? "PreToolUse" : dname === "post-tools.d" ? "PostToolUse" : "SessionStart";
      for (const abs of listShFiles(d)) {
        add({
          id: makeId(plugin, kind, event, basename(abs)),
          sourcePath: rel(abs),
          plugin,
          kind,
          event,
          matcher: "",
          commandOrModule: basename(abs),
          parentId: null,
        });
      }
    }

    for (const name of [
      "register.sh",
      "session-start.sh",
      "check-toolu.sh",
      "user-prompt-submit.sh",
      "pre-compact.sh",
    ]) {
      const abs = join(ROOT, "plugins", plugin, "hooks", name);
      if (!existsSync(abs)) continue;
      const event =
        name === "user-prompt-submit.sh"
          ? "UserPromptSubmit"
          : name === "pre-compact.sh"
            ? "PreCompact"
            : "SessionStart";
      add({
        id: makeId(plugin, "entrypoint", event, name),
        sourcePath: rel(abs),
        plugin,
        kind: "entrypoint",
        event,
        matcher: "",
        commandOrModule: name,
        parentId: null,
      });
    }
  }

  for (const sub of ["pre-tools/modules", "post-tools/modules"]) {
    const d = join(ROOT, "plugins/toolu/hooks", sub);
    const event = sub.startsWith("pre-") ? "PreToolUse" : "PostToolUse";
    for (const abs of listShFiles(d)) {
      add({
        id: makeId("toolu", "builtin-module", event, basename(abs)),
        sourcePath: rel(abs),
        plugin: "toolu",
        kind: "builtin-module",
        event,
        matcher: "",
        commandOrModule: basename(abs),
        parentId: `toolu:hooks.json:${event}:mod.sh`,
      });
    }
  }

  const agentTier = join(ROOT, "plugins/toolu/hooks/pre-tools/agent-tier.sh");
  if (existsSync(agentTier)) {
    add({
      id: makeId("toolu", "entrypoint", "PreToolUse", "agent-tier.sh"),
      sourcePath: rel(agentTier),
      plugin: "toolu",
      kind: "entrypoint",
      event: "PreToolUse",
      matcher: "",
      commandOrModule: "agent-tier.sh",
      parentId: null,
    });
  }

  for (const abs of listShFiles(join(ROOT, "plugins/toolu/hooks/lib"))) {
    add({
      id: makeId("toolu", "lib", "dependency", basename(abs)),
      sourcePath: rel(abs),
      plugin: "toolu",
      kind: "lib",
      event: "dependency",
      matcher: "",
      commandOrModule: basename(abs),
      parentId: null,
    });
  }

  return out.sort((a, b) => a.id.localeCompare(b.id));
}

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
    default:
      return `Registry ${d.kind} module ${d.commandOrModule}.`;
  }
}

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
  const bashRequired = true;

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
  if (d.kind === "entrypoint" && d.event === "SessionStart" && d.commandOrModule === "session-start.sh") {
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
    bashRequired,
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

function loadInventory(): InventoryRow[] {
  if (!existsSync(INVENTORY)) fail(`missing inventory ${INVENTORY}`);
  const data = JSON.parse(readFileSync(INVENTORY, "utf8"));
  if (!Array.isArray(data)) fail("inventory must be a JSON array");
  return data as InventoryRow[];
}

function check(): void {
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
  console.log("gate-coverage-inventory: ok");
}

function render(rows: InventoryRow[]): void {
  const header = `# Gate coverage matrix

**Issue:** [#209](https://github.com/Falconiere/toolu/issues/209) (epic [#203](https://github.com/Falconiere/toolu/issues/203))  
**Inventory:** \`tooling/fixtures/gate-coverage/inventory.json\`  
**Check:** \`bun run tooling/gate-coverage-inventory.ts check\`

Classifications match [docs/portable-core.md](portable-core.md): \`shell-out\` · \`port-native\` · \`port-new\` · \`no-map\`.

| id | source | plugin | event | classification | support | impl | host mechanism | bash | limits |
|----|--------|--------|-------|----------------|---------|------|----------------|------|--------|
`;
  const lines = rows
    .slice()
    .sort((a, b) => a.id.localeCompare(b.id))
    .map((r) => {
      const impl =
        r.implementationIssue == null ? r.implementationStatus : `#${r.implementationIssue}/${r.implementationStatus}`;
      return `| \`${r.id}\` | \`${r.sourcePath}\` | ${r.plugin} | ${r.event} | ${r.classification} | ${r.support} | ${impl} | ${r.hostMechanism} | ${r.bashRequired ? "yes" : "no"} | ${r.limits || "—"} |`;
    });
  const body =
    header +
    lines.join("\n") +
    "\n\n## Notes\n\n- Concern fragments are inventoried with `parentId` pointing at `register.sh` when present; they are not independently executable.\n- `hostMechanism=pending-opencode` is resolved against OpenCode pins during #204/#212.\n- Verification conformance links are filled by #212.\n";
  writeFileSync(MATRIX, body);
  console.log(`gate-coverage-inventory: wrote ${rel(MATRIX)} (${rows.length} rows)`);
}

function seed(): void {
  const discovered = discover();
  const rows = discovered.map(defaultMeta);
  mkdirSync(join(ROOT, "tooling/fixtures/gate-coverage"), { recursive: true });
  writeFileSync(INVENTORY, `${JSON.stringify(rows, null, 2)}\n`);
  console.log(`gate-coverage-inventory: seeded ${rel(INVENTORY)} (${rows.length} rows)`);
  render(rows);
}

const cmd = process.argv[2] ?? "check";
switch (cmd) {
  case "discover":
    console.log(JSON.stringify(discover(), null, 2));
    break;
  case "check":
    check();
    break;
  case "render":
    render(loadInventory());
    break;
  case "seed":
    seed();
    break;
  default:
    fail(`unknown command ${cmd} (discover|check|render|seed)`);
}
