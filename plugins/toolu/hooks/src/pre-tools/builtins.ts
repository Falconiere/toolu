/**
 * The toolu plugin's built-in PreToolUse modules (#258), in the byte order
 * `pre-tools/mod.sh` globbed `modules/*.sh`. A ported module (#260–#262) is
 * its native gate from `@toolu/core/gates`; the rest still run their bash
 * script.
 */
import { dirname, join } from "node:path";
import { bashModule, type ToolModule } from "@toolu/core/dispatch";
import {
  bashCommandsModule,
  commitGateModule,
  qualityGateModule,
  type GateModuleOptions,
} from "@toolu/core/gates";

export const BUILTIN_MODULES = [
  "bash-commands",
  "code-edit-rules",
  "commit-gate",
  "docs-sync",
  "mcp-blocker",
  "plan-ledger",
  "protected-files",
  "push-review",
  "quality-gate",
] as const;

type BuiltinName = (typeof BUILTIN_MODULES)[number];

/** The ported modules; every other name runs `modules/<name>.sh`. */
export const NATIVE_MODULES: Readonly<
  Partial<Record<BuiltinName, (options: GateModuleOptions) => ToolModule>>
> = {
  "bash-commands": bashCommandsModule,
  "commit-gate": commitGateModule,
  "quality-gate": qualityGateModule,
};

/** The dispatch table for the plugin whose `hooks/` directory is `hooksDir`. */
export function builtins(hooksDir: string): ToolModule[] {
  const modules = join(hooksDir, "pre-tools", "modules");
  const options = { pluginRoot: dirname(hooksDir) };
  return BUILTIN_MODULES.map(
    (name) => NATIVE_MODULES[name]?.(options) ?? bashModule(modules, name),
  );
}
