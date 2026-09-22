import type { Host } from "../args/types";

/** One plugin's state on a host, normalized so both hosts yield one shape. */
export interface InstalledPlugin {
  readonly name: string;
  readonly marketplace: string;
  readonly version: string;
  readonly enabled: boolean;
  readonly path: string | undefined;
}

export interface HostCommand {
  readonly argv: readonly string[];
}

export interface HostAdapter {
  readonly host: Host;
  readonly bin: string;
  addMarketplace(source: string): HostCommand;
  install(name: string, marketplace: string, scope: string | undefined): HostCommand;
  remove(name: string, marketplace: string): HostCommand;
  update(name: string, marketplace: string): HostCommand;
  listInstalled(): HostCommand;
  listAvailable(): HostCommand;
  parseList(stdout: string): readonly InstalledPlugin[];
}
