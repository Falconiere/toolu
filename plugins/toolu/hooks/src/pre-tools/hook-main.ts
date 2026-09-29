/**
 * Shared shell of the PreToolUse (#258) and PostToolUse (#259) bundles: read
 * the hook's stdin, run it, relay stdout, stderr and the exit code. A
 * dispatcher bug must not become a silent allow, so an unexpected error exits
 * 2: before a tool it blocks the call, like the launcher does when Bun is
 * missing; after one the host shows the model that the post-tool checks did
 * not run.
 */
import { dirname } from "node:path";
import type { ModuleResult } from "@toolu/core/dispatch";

type Run = (stdin: string, hooksDir: string) => ModuleResult | Promise<ModuleResult>;

/** `entryDir` is the entry's `import.meta.dir`: `hooks/src` from source, `hooks/dist` bundled. */
export async function hookMain(
  entryDir: string,
  run: Run,
  event: "PreToolUse" | "PostToolUse" = "PreToolUse",
): Promise<void> {
  try {
    const result = await run(await Bun.stdin.text(), dirname(entryDir));
    process.stdout.write(result.stdout);
    process.stderr.write(result.stderr);
    process.exitCode = result.exitCode;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const prefix = event === "PreToolUse" ? "blocked: " : "";
    process.stderr.write(`${prefix}toolu ${event} dispatcher failed: ${message}\n`);
    process.exitCode = 2;
  }
}
