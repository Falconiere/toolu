/** Discover hook/behavior sources from the live plugins tree. */
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
import { fail, listDirNames, makeId, normalizeCommand, rel } from "./fs-util.ts";
import { NATIVE_MODULES } from "../../../plugins/toolu/hooks/src/pre-tools/builtins.ts";
import { ROOT } from "./paths.ts";
import type { Discovered } from "./types.ts";

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

/**
 * Built-in PreToolUse modules ported to native TypeScript (#260–#262): their
 * script is gone, so each is found in the plugin's native table and sourced
 * from its `@toolu/core/gates` file.
 */
function discoverNativeBuiltins(add: AddFn): void {
  for (const name of Object.keys(NATIVE_MODULES).toSorted()) {
    const abs = join(ROOT, "packages/toolu-core/src/gates", `${name}.ts`);
    if (!existsSync(abs)) fail(`native built-in module ${name} has no ${rel(abs)}`);
    add({
      id: makeId("toolu", "builtin-module", "PreToolUse", name),
      sourcePath: rel(abs),
      plugin: "toolu",
      kind: "builtin-module",
      event: "PreToolUse",
      matcher: "",
      commandOrModule: name,
      parentId:
        "toolu:hooks.json:PreToolUse:pre-tools.js:apply_patch|Edit|Write|MultiEdit|Bash|Shell|Grep",
    });
  }
}

function discoverBuiltinModules(add: AddFn): void {
  discoverNativeBuiltins(add);
  for (const name of ["gate-status", "push-waiver"]) {
    const abs = join(ROOT, "packages/toolu-core/src/gates", `${name}.ts`);
    if (!existsSync(abs)) fail(`native built-in module ${name} has no ${rel(abs)}`);
    add({
      id: makeId("toolu", "builtin-module", "PostToolUse", name),
      sourcePath: rel(abs),
      plugin: "toolu",
      kind: "builtin-module",
      event: "PostToolUse",
      matcher: "",
      commandOrModule: name,
      parentId:
        "toolu:hooks.json:PostToolUse:post-tools.js:apply_patch|Edit|Write|MultiEdit|Bash|Sh",
    });
  }
  const agentTier = join(ROOT, "plugins/toolu/hooks/src/agent-tier.ts");
  if (existsSync(agentTier)) {
    add({
      id: makeId("toolu", "entrypoint", "PreToolUse", "agent-tier"),
      sourcePath: rel(agentTier),
      plugin: "toolu",
      kind: "entrypoint",
      event: "PreToolUse",
      matcher: "",
      commandOrModule: "agent-tier",
      parentId: "toolu:hooks.json:PreToolUse:agent-tier.js:spawn_agent|Agent|Task",
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
  }
  discoverBuiltinModules(add);
  return out.toSorted((a, b) => a.id.localeCompare(b.id));
}
