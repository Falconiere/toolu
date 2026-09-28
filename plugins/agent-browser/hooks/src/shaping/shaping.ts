/**
 * Token-lean defaults for the read-heavy agent-browser commands. Injection is
 * fail-open via a STATIC per-subcommand allow-map: a default flag is added ONLY
 * where that subcommand accepts it, so shaping can never turn a working call
 * into a usage error; every other subcommand passes through untouched.
 */
const DEFAULT_MAX_OUTPUT = "4000";

/** Exact-token or `--opt=value` match against the caller's args. */
function hasFlag(needle: string, args: readonly string[]): boolean {
  return args.some((arg) => arg === needle || arg.startsWith(`${needle}=`));
}

/** The flags to insert between the subcommand and the caller's own args. */
export function injectedFlags(command: string, args: readonly string[]): string[] {
  const inject: string[] = [];
  const add = (flag: string, ...value: string[]): void => {
    if (!hasFlag(flag, args)) inject.push(flag, ...value);
  };
  switch (command) {
    case "snapshot":
      // -i (interactive-only compact tree) unless the caller already scoped it.
      if (!["-i", "-a", "-s", "-d"].some((scope) => hasFlag(scope, args))) inject.push("-i");
      add("--json");
      add("--max-output", DEFAULT_MAX_OUTPUT);
      add("--content-boundaries");
      break;
    case "get":
      add("--json");
      add("--max-output", DEFAULT_MAX_OUTPUT);
      add("--content-boundaries");
      break;
    case "find":
    case "diff":
      add("--json");
      add("--max-output", DEFAULT_MAX_OUTPUT);
      break;
    default:
      // Unknown or binary-output subcommand: inject nothing.
      break;
  }
  return inject;
}
