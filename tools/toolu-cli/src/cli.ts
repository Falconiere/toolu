#!/usr/bin/env node
import { constants } from "node:fs";
import { access, readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "./args/parse";
import { CliError, EXIT, UsageError, type ExitCode } from "./exit";
import { dispatchPlugins } from "./plugins/dispatch";

const HELP = `toolu <noun> <verb> [options]

Install toolu plugins and Codex agent profiles.

Commands:
  plugins install [name...]   Install plugins, core first (no names = all)
  plugins list                Show catalog and installation state
  plugins remove <name...>    Uninstall plugins
  plugins update [name...]    Update plugins to the marketplace version
  agents preview              Show the Codex agent-profile plan, writing nothing
  agents install              Install Codex agent profiles
  agents remove --yes         Remove Codex agent profiles

Options:
  --host <id>       claude | codex | opencode (detected when omitted)
  --scope <scope>   user | project | local (Claude Code only)
  --config <path>   Replay a .toolu/plugins.json selection
  --yes, -y         Confirm destructive or command-declaring operations
  --force           Replace an unmanaged conflicting file, after backup
  --dry-run         Print the host commands without running them
  --no-input        Never prompt; fail listing what is missing
  --json            Machine-readable output where supported
  -h, --help        Show help
  -v, --version     Show the version
`;

async function packageVersion(): Promise<string> {
  const here = dirname(fileURLToPath(import.meta.url));
  for (const candidate of ["../package.json", "../../package.json"]) {
    try {
      const raw: unknown = JSON.parse(await readFile(resolve(here, candidate), "utf8"));
      if (typeof raw === "object" && raw !== null && "version" in raw) {
        const { version } = raw as { version: unknown };
        if (typeof version === "string") return version;
      }
    } catch {
      continue;
    }
  }
  throw new Error("package version is missing");
}

/** The CLI prompts only on a real terminal, and never when --no-input is passed. */
function isInteractive(noInput: boolean): boolean {
  return !noInput && process.stdin.isTTY === true && process.stdout.isTTY === true;
}

/**
 * Locates the marketplace manifest.
 *
 * The published package carries its own copy under assets/, because there is no
 * repository beside an installed tarball. Running from source in this repo, that
 * copy is absent and the real manifest four levels up is used instead.
 */
async function manifestPath(): Promise<string> {
  const here = dirname(fileURLToPath(import.meta.url));
  const candidates = [
    resolve(here, "../assets/marketplace.json"),
    resolve(here, "../../assets/marketplace.json"),
    resolve(here, "../../../.claude-plugin/marketplace.json"),
  ];
  for (const candidate of candidates) {
    if (await readable(candidate)) return candidate;
  }
  throw new Error("marketplace manifest not found beside the CLI or in the repository");
}

async function readable(path: string): Promise<boolean> {
  try {
    await access(path, constants.R_OK);
    return true;
  } catch {
    return false;
  }
}

export async function main(argv: readonly string[] = process.argv.slice(2)): Promise<ExitCode> {
  try {
    const args = parseArgs(argv);
    if (args.version) {
      process.stdout.write(`${await packageVersion()}\n`);
      return EXIT.ok;
    }
    if (args.help || args.noun === undefined) {
      process.stdout.write(HELP);
      return args.noun === undefined && !args.help ? EXIT.usage : EXIT.ok;
    }
    if (args.noun === "plugins") {
      return await dispatchPlugins(args, {
        manifestPath: await manifestPath(),
        interactive: isInteractive(args.noInput),
        write: (text: string) => process.stdout.write(text),
      });
    }
    throw new UsageError(`${args.noun} ${args.verb ?? ""} is not implemented yet`.trim());
  } catch (error) {
    if (error instanceof CliError) {
      process.stderr.write(`toolu: ${error.message}\n`);
      return error.code;
    }
    process.stderr.write(`toolu: ${error instanceof Error ? error.message : String(error)}\n`);
    return EXIT.failed;
  }
}

if (import.meta.main === true) process.exitCode = await main();
