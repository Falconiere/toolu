import { dependentsOf, installOrder } from "../catalog/order";
import type { Marketplace } from "../catalog/types";
import { run } from "../host/run";
import { firstLine } from "../text";
import type { HostAdapter, InstalledPlugin } from "../host/types";

type StepOutcome = "installed" | "already" | "skew" | "failed" | "skipped";

export interface InstallStep {
  readonly name: string;
  readonly outcome: StepOutcome;
  readonly detail: string;
  readonly argv: readonly string[];
}

interface InstallOptions {
  readonly adapter: HostAdapter;
  readonly marketplace: Marketplace;
  readonly marketplaceName: string;
  readonly marketplaceSource: string;
  readonly requested: readonly string[];
  readonly scope: string | undefined;
  readonly dryRun: boolean;
  readonly env?: NodeJS.ProcessEnv;
}

const CORE = "toolu";

async function versionsFrom(
  adapter: HostAdapter,
  argv: readonly string[],
  env: NodeJS.ProcessEnv | undefined,
): Promise<Map<string, InstalledPlugin>> {
  const result = await run([...argv], env);
  if (result.code !== 0) return new Map<string, InstalledPlugin>();
  try {
    return new Map(adapter.parseList(result.stdout).map((entry) => [entry.name, entry]));
  } catch {
    return new Map<string, InstalledPlugin>();
  }
}

function presentStep(
  name: string,
  present: InstalledPlugin,
  offered: InstalledPlugin | undefined,
  argv: readonly string[],
): InstallStep {
  if (offered === undefined || offered.version === present.version) {
    return { name, outcome: "already", detail: `already installed at ${present.version}`, argv };
  }
  return {
    name,
    outcome: "skew",
    detail: `installed at ${present.version}, marketplace offers ${offered.version}; left untouched. Run \`toolu plugins update ${name}\` to change it.`,
    argv,
  };
}

/**
 * Installs the requested plugins in dependency order, adding the marketplace first.
 *
 * A failure of the core plugin stops its dependents, because they cannot succeed
 * without it; they are reported as skipped. Any other failure is recorded and the
 * remaining plugins are still attempted.
 */
export async function installPlugins(options: InstallOptions): Promise<readonly InstallStep[]> {
  const order = installOrder(options.marketplace, options.requested);
  if (!options.dryRun) {
    await run([...options.adapter.addMarketplace(options.marketplaceSource).argv], options.env);
  }
  const installed = await versionsFrom(
    options.adapter,
    options.adapter.listInstalled().argv,
    options.env,
  );
  const offered = await versionsFrom(
    options.adapter,
    options.adapter.listAvailable().argv,
    options.env,
  );
  const coreDependents = new Set(dependentsOf(options.marketplace, CORE));
  const steps: InstallStep[] = [];
  let coreFailed = false;
  for (const name of order) {
    const { argv } = options.adapter.install(name, options.marketplaceName, options.scope);
    if (coreFailed && coreDependents.has(name)) {
      steps.push({ name, outcome: "skipped", detail: `skipped: ${CORE} failed`, argv });
      continue;
    }
    const present = installed.get(name);
    if (present !== undefined) {
      steps.push(presentStep(name, present, offered.get(name), argv));
      continue;
    }
    if (options.dryRun) {
      steps.push({ name, outcome: "skipped", detail: "dry run", argv });
      continue;
    }
    const result = await run([...argv], options.env);
    const ok = result.code === 0;
    steps.push({
      name,
      outcome: ok ? "installed" : "failed",
      detail: ok ? "installed" : firstLine(result.stderr, "install failed"),
      argv,
    });
    if (!ok && name === CORE) coreFailed = true;
  }
  return steps;
}
