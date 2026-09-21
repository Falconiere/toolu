/** Discover hook/behavior sources from the live plugins tree. */
import { existsSync, readFileSync } from "node:fs";
import { basename, join } from "node:path";
import { z } from "zod";
import { listDirNames, listShFiles, makeId, normalizeCommand, rel } from "./fs-util.ts";
import { ROOT } from "./paths.ts";
import type { Discovered, Kind } from "./types.ts";

const HookCommandSchema = z.object({ command: z.string().optional() }).passthrough();
const HookEntrySchema = z
  .object({
    matcher: z.string().optional(),
    hooks: z.array(HookCommandSchema).optional(),
    command: z.string().optional(),
  })
  .passthrough();
const HooksFileSchema = z
  .object({ hooks: z.record(z.string(), z.array(HookEntrySchema)).optional() })
  .passthrough();

type AddFn = (row: Discovered) => void;

function discoverHooksJson(plugin: string, add: AddFn): void {
  const hooksJson = join(ROOT, "plugins", plugin, "hooks", "hooks.json");
  if (!existsSync(hooksJson)) return;
  const parsed = HooksFileSchema.safeParse(JSON.parse(readFileSync(hooksJson, "utf8")));
  if (!parsed.success) return;
  const hooks = parsed.data.hooks ?? {};
  for (const [event, entries] of Object.entries(hooks)) {
    for (const entry of entries) {
      const matcher = entry.matcher ?? "";
      const commands: string[] = [];
      if (entry.hooks) {
        for (const h of entry.hooks) {
          if (h.command) commands.push(h.command);
        }
      } else if (entry.command) {
        commands.push(entry.command);
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

function discoverConcerns(plugin: string, add: AddFn): void {
  const concernsDir = join(ROOT, "plugins", plugin, "hooks", "concerns");
  const parentId = `${plugin}:entrypoint:SessionStart:register.sh`;
  const hasRegister = existsSync(join(ROOT, "plugins", plugin, "hooks", "register.sh"));
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
      parentId: hasRegister ? parentId : null,
    });
  }
}

function discoverEventDirs(plugin: string, add: AddFn): void {
  for (const dname of ["pre-tools.d", "post-tools.d", "session-start.d"] as const) {
    const d = join(ROOT, "plugins", plugin, "hooks", dname);
    const kind: Kind = dname;
    const event =
      dname === "pre-tools.d"
        ? "PreToolUse"
        : dname === "post-tools.d"
          ? "PostToolUse"
          : "SessionStart";
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
}

function discoverEntrypoints(plugin: string, add: AddFn): void {
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

function discoverBuiltinModules(add: AddFn): void {
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
}

/** Discover all inventory rows from the repository plugins tree. */
export function discover(): Discovered[] {
  const out: Discovered[] = [];
  const seen = new Set<string>();
  const add: AddFn = (row) => {
    let id = row.id;
    if (seen.has(id)) id = `${id}#${seen.size}`;
    seen.add(id);
    out.push({ ...row, id });
  };
  for (const plugin of listDirNames(join(ROOT, "plugins"))) {
    discoverHooksJson(plugin, add);
    discoverConcerns(plugin, add);
    discoverEventDirs(plugin, add);
    discoverEntrypoints(plugin, add);
  }
  discoverBuiltinModules(add);
  return out.toSorted((a, b) => a.id.localeCompare(b.id));
}
