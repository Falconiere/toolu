import type { ParsedArgs, Verb } from "../args/types";
import { readMarketplace } from "../catalog/manifest";
import type { Marketplace } from "../catalog/types";
import { assertScopeAllowed } from "../args/parse";
import { adapterFor, resolveHost } from "../host/detect";
import { CliError, EXIT, UsageError, type ExitCode } from "../exit";
import { installPlugins } from "./install";
import { listPlugins } from "./list";
import { removePlugins } from "./remove";
import { updatePlugins } from "./update";
import { anyFailed, reportInstall, reportList, reportRemove, reportUpdate } from "../ui/report";

const MARKETPLACE_NAME = "toolu";
const MARKETPLACE_SOURCE = "Falconiere/toolu";

/** Parsed arguments once a verb is known to be present. */
type RoutedArgs = ParsedArgs & { readonly verb: Verb };

interface DispatchContext {
  readonly manifestPath: string;
  readonly interactive: boolean;
  readonly write: (text: string) => void;
}

async function handleInstall(
  args: ParsedArgs,
  marketplace: Marketplace,
  context: DispatchContext,
): Promise<ExitCode> {
  const host = await resolvedHost(args);
  const steps = await installPlugins({
    adapter: adapterFor(host),
    marketplace,
    marketplaceName: MARKETPLACE_NAME,
    marketplaceSource: MARKETPLACE_SOURCE,
    requested: args.names,
    scope: args.scope,
    dryRun: args.dryRun,
  });
  context.write(reportInstall(steps, args.dryRun));
  return anyFailed(steps) ? EXIT.failed : EXIT.ok;
}

/** Resolves the host and applies the checks every verb shares. */
async function resolvedHost(args: ParsedArgs): Promise<"claude" | "codex"> {
  const { host } = await resolveHost(args.host);
  if (host === "opencode") {
    throw new UsageError(
      "OpenCode is not wired into the CLI yet. Install the bridge with `opencode plugin add @toolu/opencode`; see docs/opencode.md",
    );
  }
  assertScopeAllowed(args.scope, host);
  return host;
}

async function handleRemove(args: ParsedArgs, context: DispatchContext): Promise<ExitCode> {
  if (args.names.length === 0) throw new UsageError("remove requires at least one name");
  if (!args.yes) {
    throw new CliError(EXIT.missingInput, "remove requires --yes to confirm");
  }
  const host = await resolvedHost(args);
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
  const host = await resolvedHost(args);
  if (args.verb === "list") {
    const entries = await listPlugins(adapterFor(host), marketplace);
    context.write(args.json ? `${JSON.stringify(entries, null, 2)}\n` : reportList(entries));
    return EXIT.ok;
  }
  const steps = await updatePlugins(adapterFor(host), MARKETPLACE_NAME, args.names);
  context.write(reportUpdate(steps));
  return anyFailed(steps) ? EXIT.failed : EXIT.ok;
}
