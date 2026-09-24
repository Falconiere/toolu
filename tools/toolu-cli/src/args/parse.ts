import { UsageError } from "../exit";
import type { z } from "zod";
import {
  HOSTS,
  SCOPES,
  VERBS,
  hostSchema,
  scopeSchema,
  verbSchema,
  type Host,
  type ParsedArgs,
  type Scope,
  type Verb,
} from "./types";

const BOOLEAN_FLAGS = new Set([
  "--yes",
  "-y",
  "--dry-run",
  "--no-input",
  "--json",
  "--help",
  "-h",
  "--version",
  "-v",
]);

const VALUE_FLAGS = new Set(["--host", "--scope", "--config"]);

interface Collected {
  readonly positionals: string[];
  readonly booleans: Set<string>;
  readonly values: Map<string, string>;
}

function collect(argv: readonly string[]): Collected {
  const positionals: string[] = [];
  const booleans = new Set<string>();
  const values = new Map<string, string>();
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index] ?? "";
    if (BOOLEAN_FLAGS.has(token)) {
      booleans.add(token);
      continue;
    }
    if (VALUE_FLAGS.has(token)) {
      const next = argv[index + 1];
      if (next === undefined || next.startsWith("-")) {
        throw new UsageError(`${token} requires a value`);
      }
      values.set(token, next);
      index += 1;
      continue;
    }
    if (token.startsWith("-")) throw new UsageError(`unknown flag: ${token}`);
    positionals.push(token);
  }
  return { positionals, booleans, values };
}

/**
 * The verb is the first argument: `npx @toolu/plugins install` hands the CLI
 * `install`, because the package name already says what it acts on.
 */
function readVerb(positionals: readonly string[]): Verb | undefined {
  const first = positionals[0];
  if (first === undefined) return undefined;
  if (first === "plugins") {
    throw new UsageError(
      "unknown command: plugins. The package name already says plugins: run `npx @toolu/plugins install`",
    );
  }
  const parsed = verbSchema.safeParse(first);
  if (!parsed.success) {
    throw new UsageError(`unknown command: ${first}. Expected one of: ${VERBS.join(", ")}`);
  }
  return parsed.data;
}

function readEnum<T extends string>(
  values: Map<string, string>,
  flag: string,
  schema: z.ZodType<T>,
  allowed: readonly T[],
): T | undefined {
  const raw = values.get(flag);
  if (raw === undefined) return undefined;
  const parsed = schema.safeParse(raw);
  if (!parsed.success) {
    throw new UsageError(`unknown ${flag} value: ${raw}. Expected one of: ${allowed.join(", ")}`);
  }
  return parsed.data;
}

/** Parses argv into a normalized shape, throwing UsageError (exit 2) on anything malformed. */
export function parseArgs(argv: readonly string[]): ParsedArgs {
  const { positionals, booleans, values } = collect(argv);
  const help = booleans.has("--help") || booleans.has("-h");
  const version = booleans.has("--version") || booleans.has("-v");
  // An unknown verb is a usage error even alongside --help: `toolu skills --help`
  // must not answer as though `skills` were a command.
  const verb = readVerb(positionals);
  const host = readEnum<Host>(values, "--host", hostSchema, HOSTS);
  const scope = readEnum<Scope>(values, "--scope", scopeSchema, SCOPES);
  // With an explicit host this is decidable now. With an implicit one the check
  // runs again once detection resolves, in assertScopeAllowed.
  if (scope !== undefined && host !== undefined && host !== "claude") {
    throw new UsageError(scopeRejection(host));
  }
  return {
    verb,
    names: positionals.slice(1),
    host,
    scope,
    config: values.get("--config"),
    yes: booleans.has("--yes") || booleans.has("-y"),
    dryRun: booleans.has("--dry-run"),
    noInput: booleans.has("--no-input"),
    json: booleans.has("--json"),
    help,
    version,
  };
}

/** The message used whether the host was named or detected. */
function scopeRejection(host: Host): string {
  return `--scope is Claude Code only; ${host} has no scope concept`;
}

/** Re-checks --scope once an implicit host has been resolved. */
export function assertScopeAllowed(scope: Scope | undefined, host: Host): void {
  if (scope !== undefined && host !== "claude") throw new UsageError(scopeRejection(host));
}
