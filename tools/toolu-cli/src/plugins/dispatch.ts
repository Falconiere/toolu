import type { Host, ParsedArgs, Verb } from "../args/types";
import { readMarketplace } from "../catalog/manifest";
import type { Marketplace } from "../catalog/types";
import { assertScopeAllowed } from "../args/parse";
import { adapterFor, resolveHosts } from "../host/detect";
import { CliError, EXIT, UsageError, type ExitCode } from "../exit";
import { dispatchOpencode, installOpencode, type OpencodeContext } from "../opencode/dispatch";
import { installPlugins, type InstallStep } from "./install";
import { installProgress } from "../ui/progress";
import { listPlugins } from "./list";
import { removePlugins } from "./remove";
import { updatePlugins } from "./update";
import {
  anyFailed,
  reportInstallByHost,
  reportList,
  reportRemove,
  reportUpdate,
} from "../ui/report";
import {
  selectHost as defaultSelectHost,
  selectHosts as defaultSelectHosts,
  selectPlugins as defaultSelectPlugins,
  type SelectHost,
  type SelectHosts,
  type SelectPlugins,
} from "../ui/prompts";

const MARKETPLACE_NAME = "toolu";
const MARKETPLACE_SOURCE = "Falconiere/toolu";

/** Parsed arguments once a verb is known to be present. */
type RoutedArgs = ParsedArgs & { readonly verb: Verb };

interface DispatchContext {
  readonly manifestPath: string;
  readonly interactive: boolean;
  readonly terminal?: boolean;
  readonly write: (text: string) => void;
  readonly env?: NodeJS.ProcessEnv;
  /** The CLI's own version: the `@toolu/opencode` release OpenCode installs. */
  readonly version?: string;
  readonly cwd?: string;
  readonly selectHosts?: SelectHosts;
  readonly selectHost?: SelectHost;
  readonly selectPlugins?: SelectPlugins;
}

function opencodeContext(marketplace: Marketplace, context: DispatchContext): OpencodeContext {
  if (context.version === undefined) {
    throw new CliError(EXIT.failed, "the CLI version is required for OpenCode");
  }
  return {
    marketplace,
    env: context.env ?? process.env,
    cwd: context.cwd ?? process.cwd(),
    version: context.version,
    write: context.write,
  };
}

async function hostsFor(
  args: ParsedArgs,
  context: DispatchContext,
  mode: "single" | "multi",
): Promise<readonly Host[]> {
  const hosts = await resolveHosts(args.host, {
    interactive: context.interactive,
    mode,
    ...(context.env === undefined ? {} : { env: context.env }),
    selectHosts: context.selectHosts ?? defaultSelectHosts,
    selectHost: context.selectHost ?? defaultSelectHost,
  });
  for (const host of hosts) assertScopeAllowed(args.scope, host);
  return hosts;
}

async function installOn(
  host: "claude" | "codex",
  args: ParsedArgs,
  requested: readonly string[],
  marketplace: Marketplace,
  context: DispatchContext,
): Promise<readonly InstallStep[]> {
  const progress =
    context.terminal === true && !args.dryRun && !args.json
      ? installProgress(host, context.write)
      : undefined;
  try {
    return await installPlugins({
      adapter: adapterFor(host),
      marketplace,
      marketplaceName: MARKETPLACE_NAME,
      marketplaceSource: MARKETPLACE_SOURCE,
      requested,
      scope: args.scope,
      dryRun: args.dryRun,
      ...(progress === undefined ? {} : { onProgress: progress.update }),
      ...(context.env === undefined ? {} : { env: context.env }),
    });
  } finally {
    progress?.stop();
  }
}

async function handleInstall(
  args: ParsedArgs,
  marketplace: Marketplace,
  context: DispatchContext,
): Promise<ExitCode> {
  const hosts = await hostsFor(args, context, "multi");
  let requested = args.names;
  if (requested.length === 0 && context.interactive) {
    const pick = context.selectPlugins ?? defaultSelectPlugins;
    try {
      requested = await pick(marketplace.plugins);
    } catch (error) {
      if (error instanceof CliError && error.code === EXIT.cancelled) return EXIT.cancelled;
      throw error;
    }
  }
  const sections: { host: Host; steps: readonly InstallStep[] }[] = [];
  for (const host of hosts) {
    const steps =
      host === "opencode"
        ? await installOpencode(args, requested, opencodeContext(marketplace, context))
        : await installOn(host, args, requested, marketplace, context);
    sections.push({ host, steps });
  }
  context.write(reportInstallByHost(sections, args.dryRun));
  return anyFailed(sections.flatMap((section) => section.steps)) ? EXIT.failed : EXIT.ok;
}

async function handleRemove(
  args: ParsedArgs,
  marketplace: Marketplace,
  context: DispatchContext,
): Promise<ExitCode> {
  if (args.names.length === 0) throw new UsageError("remove requires at least one name");
  if (!args.yes) {
    throw new CliError(EXIT.missingInput, "remove requires --yes to confirm");
  }
  const [host] = await hostsFor(args, context, "single");
  if (host === undefined) throw new CliError(EXIT.missingInput, "no host selected");
  if (host === "opencode") {
    return dispatchOpencode({ ...args, verb: "remove" }, opencodeContext(marketplace, context));
  }
  const steps = await removePlugins(adapterFor(host), MARKETPLACE_NAME, args.names);
  context.write(reportRemove(steps));
  return anyFailed(steps) ? EXIT.failed : EXIT.ok;
}

/** Routes a parsed invocation to its verb. */
export async function dispatchPlugins(
  args: RoutedArgs,
  context: DispatchContext,
): Promise<ExitCode> {
  const marketplace = await readMarketplace(context.manifestPath);
  if (args.verb === "install") return handleInstall(args, marketplace, context);
  if (args.verb === "remove") return handleRemove(args, marketplace, context);
  const [host] = await hostsFor(args, context, "single");
  if (host === undefined) throw new CliError(EXIT.missingInput, "no host selected");
  if (host === "opencode") {
    return dispatchOpencode({ ...args, verb: args.verb }, opencodeContext(marketplace, context));
  }
  if (args.verb === "list") {
    const entries = await listPlugins(adapterFor(host), marketplace, context.env);
    context.write(args.json ? `${JSON.stringify(entries, null, 2)}\n` : reportList(entries));
    return EXIT.ok;
  }
  const steps = await updatePlugins(adapterFor(host), MARKETPLACE_NAME, args.names);
  context.write(reportUpdate(steps));
  return anyFailed(steps) ? EXIT.failed : EXIT.ok;
}
