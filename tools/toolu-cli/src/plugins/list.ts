import { catalogNames } from "../catalog/order";
import type { Marketplace } from "../catalog/types";
import { run } from "../host/run";
import type { HostAdapter } from "../host/types";

export interface CatalogState {
  readonly name: string;
  readonly installed: boolean;
  readonly version: string | undefined;
  readonly enabled: boolean;
}

/** The catalog joined with what the host reports as installed. */
export async function listPlugins(
  adapter: HostAdapter,
  marketplace: Marketplace,
  env?: NodeJS.ProcessEnv,
): Promise<readonly CatalogState[]> {
  const result = await run([...adapter.listInstalled().argv], env);
  const present =
    result.code === 0
      ? new Map(adapter.parseList(result.stdout).map((entry) => [entry.name, entry]))
      : new Map<string, never>();
  return catalogNames(marketplace).map((name) => {
    const entry = present.get(name);
    return {
      name,
      installed: entry !== undefined,
      version: entry?.version,
      enabled: entry?.enabled ?? false,
    };
  });
}
