import { run } from "../host/run";

function firstLine(text: string): string {
  return (
    text
      .split("\n")
      .find((line) => line.trim().length > 0)
      ?.trim() ?? "remove failed"
  );
}
import type { HostAdapter } from "../host/types";

export interface RemoveStep {
  readonly name: string;
  readonly removed: boolean;
  readonly detail: string;
  readonly argv: readonly string[];
}

/** Uninstalls the named plugins, reporting each independently. */
export async function removePlugins(
  adapter: HostAdapter,
  marketplaceName: string,
  names: readonly string[],
  env?: NodeJS.ProcessEnv,
): Promise<readonly RemoveStep[]> {
  const steps: RemoveStep[] = [];
  for (const name of names) {
    const { argv } = adapter.remove(name, marketplaceName);
    const result = await run([...argv], env);
    steps.push({
      name,
      removed: result.code === 0,
      detail: result.code === 0 ? "removed" : firstLine(result.stderr),
      argv,
    });
  }
  return steps;
}
