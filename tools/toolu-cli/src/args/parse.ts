import { UsageError } from "../exit";
import type { z } from "zod";
import {
  AGENT_VERBS,
  HOSTS,
  NOUNS,
  PLUGIN_VERBS,
  SCOPES,
  hostSchema,
  nounSchema,
  scopeSchema,
  type Host,
  type Noun,
  type ParsedArgs,
  type Scope,
} from "./types";

const BOOLEAN_FLAGS = new Set([
  "--yes",
  "-y",
  "--force",
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

function readNoun(positionals: readonly string[]): Noun | undefined {
  const first = positionals[0];
  if (first === undefined) return undefined;
  const parsed = nounSchema.safeParse(first);
  if (!parsed.success) {
    throw new UsageError(`unknown command: ${first}. Expected one of: ${NOUNS.join(", ")}`);
  }
  return parsed.data;
}

function readVerb(noun: Noun | undefined, positionals: readonly string[]): string | undefined {
  if (noun === undefined) return undefined;
  const verb = positionals[1];
  const allowed: readonly string[] = noun === "plugins" ? PLUGIN_VERBS : AGENT_VERBS;
  if (verb === undefined) throw new UsageError(`${noun} requires a verb: ${allowed.join(", ")}`);
  if (!allowed.includes(verb)) {
    throw new UsageError(`unknown ${noun} verb: ${verb}. Expected one of: ${allowed.join(", ")}`);
  }
  return verb;
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
  // An unknown noun is a usage error even alongside --help: `toolu skills --help`
  // must not answer as though `skills` were a command.
  const noun = readNoun(positionals);
  const verb = help || version ? undefined : readVerb(noun, positionals);
  const host = readEnum<Host>(values, "--host", hostSchema, HOSTS);
  const scope = readEnum<Scope>(values, "--scope", scopeSchema, SCOPES);
  // With an explicit host this is decidable now. With an implicit one the check
  // runs again once detection resolves, in assertScopeAllowed.
  if (scope !== undefined && host !== undefined && host !== "claude") {
    throw new UsageError(scopeRejection(host));
  }
  return {
    noun,
    verb,
    names: positionals.slice(2),
    host,
    scope,
    config: values.get("--config"),
    yes: booleans.has("--yes") || booleans.has("-y"),
    force: booleans.has("--force"),
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
