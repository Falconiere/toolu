/**
 * The bash families' `while case` flag loops: a value flag takes the next
 * argument (a missing one is exit 1, where bash died on `shift 2`), a repeated
 * value flag keeps the last value, a list flag accumulates, and anything else
 * goes to `other`, which decides between a positional and "unknown option".
 */
import { CliExit, flagValue } from "@toolu/core/cli";

export interface FlagSpec {
  /** Spelling → option name, for flags that take one value. */
  readonly values?: Readonly<Record<string, string>>;
  /** Spelling → option name, for repeatable flags (`-f field=val`). */
  readonly lists?: Readonly<Record<string, string>>;
  /** Spelling → option name, for flags without a value. */
  readonly switches?: Readonly<Record<string, string>>;
}

export interface Flags {
  readonly values: Map<string, string>;
  readonly lists: Map<string, string[]>;
  readonly switches: Set<string>;
}

function lookup(table: Readonly<Record<string, string>> | undefined, arg: string) {
  // Own keys only: an argument such as "constructor" is not a flag.
  return table !== undefined && Object.hasOwn(table, arg) ? table[arg] : undefined;
}

/** Exit 1 with bash's `<context>: unknown option '<arg>'`. */
export function unknownOption(context: string, arg: string): never {
  throw new CliExit(1, `${context}: unknown option '${arg}'`);
}

export function readFlags(
  argv: readonly string[],
  spec: FlagSpec,
  other: (arg: string, flags: Flags) => void,
): Flags {
  const flags: Flags = { values: new Map(), lists: new Map(), switches: new Set() };
  for (let at = 0; at < argv.length; at += 1) {
    const arg = argv[at] ?? "";
    const value = lookup(spec.values, arg);
    const list = lookup(spec.lists, arg);
    const toggle = lookup(spec.switches, arg);
    if (value !== undefined) {
      flags.values.set(value, flagValue("jira", argv, at));
      at += 1;
    } else if (list !== undefined) {
      flags.lists.set(list, [...(flags.lists.get(list) ?? []), flagValue("jira", argv, at)]);
      at += 1;
    } else if (toggle !== undefined) {
      flags.switches.add(toggle);
    } else {
      other(arg, flags);
    }
  }
  return flags;
}

/** Flags where every other argument is an unknown option. */
export function onlyFlags(context: string, argv: readonly string[], spec: FlagSpec): Flags {
  return readFlags(argv, spec, (arg) => unknownOption(context, arg));
}

/** `${1:-}` then `shift`: the first argument (empty when absent) and the rest. */
export function shiftArg(argv: readonly string[]): [string, string[]] {
  const [first = "", ...rest] = argv;
  return [first, rest];
}

export type Action<C> = (ctx: C, argv: readonly string[]) => Promise<number>;

/** `case "$action" in …`: run the named action, or exit 1 with the family usage. */
export function route<C>(
  usage: string,
  actions: Readonly<Record<string, Action<C>>>,
  ctx: C,
  argv: readonly string[],
): Promise<number> {
  const [action, rest] = shiftArg(argv);
  const run = Object.hasOwn(actions, action) ? actions[action] : undefined;
  if (run === undefined) throw new CliExit(1, usage);
  return run(ctx, rest);
}
