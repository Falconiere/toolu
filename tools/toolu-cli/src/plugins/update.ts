import { CliError, EXIT } from "../exit";
import { run } from "../host/run";
import type { HostAdapter } from "../host/types";

export interface UpdateStep {
  readonly name: string;
  readonly outcome: "updated" | "current" | "failed";
  readonly detail: string;
  readonly argv: readonly string[];
}

async function versions(
  adapter: HostAdapter,
  argv: readonly string[],
  env: NodeJS.ProcessEnv | undefined,
): Promise<Map<string, string>> {
  const result = await run([...argv], env);
  if (result.code !== 0) return new Map<string, string>();
  try {
    return new Map(adapter.parseList(result.stdout).map((entry) => [entry.name, entry.version]));
  } catch {
    return new Map<string, string>();
  }
}

/** Updates only the plugins whose installed version differs from the offered one. */
export async function updatePlugins(
  adapter: HostAdapter,
  marketplaceName: string,
  names: readonly string[],
  env?: NodeJS.ProcessEnv,
): Promise<readonly UpdateStep[]> {
  const installed = await versions(adapter, adapter.listInstalled().argv, env);
  const offered = await versions(adapter, adapter.listAvailable().argv, env);
  const targets = names.length > 0 ? names : [...installed.keys()];
  if (targets.length === 0) {
    // An empty request plus an empty installed set means the host told us
    // nothing. Saying so beats returning no steps and reading as success.
    throw new CliError(
      EXIT.failed,
      `${adapter.bin} reported no installed plugins, so there is nothing to update`,
    );
  }
  const steps: UpdateStep[] = [];
  for (const name of targets) {
    const { argv } = adapter.update(name, marketplaceName);
    const have = installed.get(name);
    const want = offered.get(name);
    if (have !== undefined && want !== undefined && have === want) {
      steps.push({ name, outcome: "current", detail: `current at ${have}`, argv });
      continue;
    }
    const result = await run([...argv], env);
    steps.push({
      name,
      outcome: result.code === 0 ? "updated" : "failed",
      detail:
        result.code === 0
          ? want === undefined
            ? "updated"
            : `updated to ${want}`
          : "update failed",
      argv,
    });
  }
  return steps;
}
