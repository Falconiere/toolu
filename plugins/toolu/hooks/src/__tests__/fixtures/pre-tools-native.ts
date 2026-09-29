/**
 * Benchmark stand-in for a ported module (#258, AC-5): the `pre-tools` entry
 * with `mcp-blocker` switched from its bash fallback to a native module that
 * always allows. The bash module is silent for every tool that is not
 * `mcp__<server>__<tool>`, and the benchmark slice has none, so there the
 * native module decides exactly what the bash one did. The real port is #260.
 * `tooling/src/benchmarks/pre-tools-latency.ts` bundles it with `HOOKS_DIR`
 * defined, since the bundle runs from a temp directory.
 */
import { join } from "node:path";
import { dispatchPreTool, type PreToolModule } from "@toolu/core/dispatch";
import { builtins } from "../../pre-tools/builtins.ts";
import { hookMain } from "../../pre-tools/hook-main.ts";

declare const HOOKS_DIR: string;

const nativeMcpBlocker: PreToolModule = {
  kind: "native",
  name: "mcp-blocker",
  run: () => Promise.resolve({ kind: "allow" }),
};

const table = builtins(HOOKS_DIR).map((module) =>
  module.name === "mcp-blocker.sh" ? nativeMcpBlocker : module,
);

await hookMain(join(HOOKS_DIR, "dist"), (stdin, hooks) =>
  dispatchPreTool(stdin, { builtins: table, libDir: join(hooks, "lib") }),
);
