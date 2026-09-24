import type { Host, ParsedArgs, Verb } from "../args/types";
import { readMarketplace } from "../catalog/manifest";
import type { Marketplace } from "../catalog/types";
import { assertScopeAllowed } from "../args/parse";
import { adapterFor, resolveHosts } from "../host/detect";
import { CliError, EXIT, UsageError, type ExitCode } from "../exit";
import { installPlugins } from "./install";
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

export interface DispatchContext {
  readonly manifestPath: string;
  readonly interactive: boolean;
  readonly write: (text: string) => void;
  readonly env?: NodeJS.ProcessEnv;
  readonly selectHosts?: SelectHosts;
  readonly selectHost?: SelectHost;
  readonly selectPlugins?: SelectPlugins;
}

function assertWired(host: Host): "claude" | "codex" {
  if (host === "opencode") {
    throw new UsageError(
      "OpenCode is not wired into the CLI yet. Install the bridge with `opencode plugin add @toolu/opencode`; see docs/opencode.md",
    );
  }
  return host;
}

async function hostsFor(
  args: ParsedArgs,
  context: DispatchContext,
  mode: "single" | "multi",
): Promise<readonly ("claude" | "codex")[]> {
  const hosts = await resolveHosts(args.host, {
    interactive: context.interactive,
    mode,
    env: context.env,
    selectHosts: context.selectHosts ?? defaultSelectHosts,
    selectHost: context.selectHost ?? defaultSelectHost,
  });
  const wired: ("claude" | "codex")[] = [];
  for (const host of hosts) {
    const id = assertWired(host);
    assertScopeAllowed(args.scope, id);
    wired.push(id);
  }
  return wired;
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
    requested = await pick(marketplace.plugins);
  }
  const sections: { host: Host; steps: Awaited<ReturnType<typeof installPlugins>> }[] = [];
  for (const host of hosts) {
    const steps = await installPlugins({
      adapter: adapterFor(host),
      marketplace,
      marketplaceName: MARKETPLACE_NAME,
      marketplaceSource: MARKETPLACE_SOURCE,
      requested,
      scope: args.scope,
      dryRun: args.dryRun,
      env: context.env,
    });
    sections.push({ host, steps });
  }
  context.write(reportInstallByHost(sections, args.dryRun));
  return anyFailed(sections.flatMap((section) => section.steps)) ? EXIT.failed : EXIT.ok;
}

async function handleRemove(args: ParsedArgs, context: DispatchContext): Promise<ExitCode> {
  if (args.names.length === 0) throw new UsageError("remove requires at least one name");
  if (!args.yes) {
    throw new CliError(EXIT.missingInput, "remove requires --yes to confirm");
  }
  const [host] = await hostsFor(args, context, "single");
  if (host === undefined) throw new CliError(EXIT.missingInput, "no host selected");
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
  if (args.verb === "remove") return handleRemove(args, context);
  const [host] = await hostsFor(args, context, "single");
  if (host === undefined) throw new CliError(EXIT.missingInput, "no host selected");
  if (args.verb === "list") {
    const entries = await listPlugins(adapterFor(host), marketplace, context.env);
    context.write(args.json ? `${JSON.stringify(entries, null, 2)}\n` : reportList(entries));
    return EXIT.ok;
  }
  const steps = await updatePlugins(adapterFor(host), MARKETPLACE_NAME, args.names);
  context.write(reportUpdate(steps));
  return anyFailed(steps) ? EXIT.failed : EXIT.ok;
}
