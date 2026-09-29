/**
 * Shared shell of the PreToolUse bundles (#258): read the hook's stdin, run
 * it, relay stdout, stderr and the exit code. A dispatcher bug must not become
 * a silent allow, so an unexpected error blocks the tool call (exit 2), like
 * the launcher does when Bun is missing.
 */
import { dirname } from "node:path";
import type { ModuleResult } from "@toolu/core/dispatch";

type Run = (stdin: string, hooksDir: string) => ModuleResult | Promise<ModuleResult>;

/** `entryDir` is the entry's `import.meta.dir`: `hooks/src` from source, `hooks/dist` bundled. */
export async function hookMain(entryDir: string, run: Run): Promise<void> {
  try {
    const result = await run(await Bun.stdin.text(), dirname(entryDir));
    process.stdout.write(result.stdout);
    process.stderr.write(result.stderr);
    process.exitCode = result.exitCode;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`blocked: toolu PreToolUse dispatcher failed: ${message}\n`);
    process.exitCode = 2;
  }
}
