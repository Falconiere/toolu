import type { ParsedArgs } from "../args/types";
import { readMarketplace } from "../catalog/manifest";
import type { Marketplace } from "../catalog/types";
import { adapterFor, resolveHost } from "../host/detect";
import { CliError, EXIT, UsageError, type ExitCode } from "../exit";
import { installPlugins } from "./install";
import { listPlugins } from "./list";
import { removePlugins } from "./remove";
import { updatePlugins } from "./update";
import { anyFailed, reportInstall, reportList, reportRemove, reportUpdate } from "../ui/report";

const MARKETPLACE_NAME = "toolu";
const MARKETPLACE_SOURCE = "Falconiere/toolu";

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
  const { host } = await resolveHost(args.host);
  if (host === "opencode") {
    throw new UsageError("OpenCode wiring is not implemented yet; see docs/opencode.md");
  }
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

async function handleRemove(args: ParsedArgs, context: DispatchContext): Promise<ExitCode> {
  if (args.names.length === 0) throw new UsageError("plugins remove requires at least one name");
  if (!args.yes) {
    throw new CliError(EXIT.missingInput, "plugins remove requires --yes to confirm");
  }
  const { host } = await resolveHost(args.host);
  const steps = await removePlugins(adapterFor(host), MARKETPLACE_NAME, args.names);
  context.write(reportRemove(steps));
  return anyFailed(steps) ? EXIT.failed : EXIT.ok;
}

/** Routes a parsed `plugins` invocation to its verb. */
export async function dispatchPlugins(
  args: ParsedArgs,
  context: DispatchContext,
): Promise<ExitCode> {
  const marketplace = await readMarketplace(context.manifestPath);
  if (args.verb === "install") return handleInstall(args, marketplace, context);
  if (args.verb === "remove") return handleRemove(args, context);
  const { host } = await resolveHost(args.host);
  if (args.verb === "list") {
    const entries = await listPlugins(adapterFor(host), marketplace);
    context.write(args.json ? `${JSON.stringify(entries, null, 2)}\n` : reportList(entries));
    return EXIT.ok;
  }
  const steps = await updatePlugins(adapterFor(host), MARKETPLACE_NAME, args.names);
  context.write(reportUpdate(steps));
  return anyFailed(steps) ? EXIT.failed : EXIT.ok;
}
