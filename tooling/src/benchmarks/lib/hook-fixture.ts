/**
 * The fixed sandbox every hook entry runs in (#410): a git project with a
 * README and a TypeScript file, toolu and the four registry plugins marked
 * installed, and their `register` SessionStart hooks run once, so the registry
 * the dispatchers load is the same on every run.
 */
import { installPlugins, registerPlugin } from "@toolu/conformance/harness/pretool";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { inSequence } from "./hook-measurer.ts";

const REGISTRY_PLUGINS = ["ast-grep", "ts-quality", "python-quality", "rust-quality"];

const FILES = {
  "README.md": "# bench\n\nhello\n",
  "src/app.ts": "export const answer = 42;\n",
};

/** Create the bench sandbox; the caller disposes of it. */
export async function benchSandbox(): Promise<Sandbox> {
  const sb = createSandbox({ git: true, files: FILES });
  try {
    // No detached gc/maintenance writing into `.git` while hooks are measured.
    sb.git("config", "maintenance.auto", "false");
    sb.git("config", "gc.auto", "0");
    installPlugins(sb, "toolu@toolu", ...REGISTRY_PLUGINS.map((plugin) => `${plugin}@toolu`));
    // One after another: register hooks write the same registry directory.
    await inSequence(REGISTRY_PLUGINS, (plugin) => registerPlugin(sb, "claude", plugin));
    return sb;
  } catch (error) {
    sb[Symbol.dispose]();
    throw error;
  }
}

/** Replace every `${PROJECT}` in the payload's strings with the sandbox project. */
export function payloadStdin(stdin: Record<string, unknown>, project: string): string {
  // Splice the JSON-escaped path, so a path needing escapes still yields valid JSON.
  return JSON.stringify(stdin).replaceAll("${PROJECT}", JSON.stringify(project).slice(1, -1));
}
