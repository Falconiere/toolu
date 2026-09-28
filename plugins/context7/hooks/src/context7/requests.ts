/** argv → Context7 request, with the bash search.sh's parsing rules and param order. */
import { CliExit, flagValue } from "@toolu/core/cli";
import { DOCS_USAGE, SEARCH_USAGE } from "./usage.ts";

const TOOL = "context7";

export interface Context7Call {
  readonly endpoint: "libs/search" | "context";
  readonly params: ReadonlyArray<readonly [string, string]>;
  /** False only for `docs -t <non-json>`, which prints the body as-is. */
  readonly json: boolean;
}

interface Parsed {
  /** The two positional slots, filled by flag or by bare argument in order. */
  first: string;
  second: string;
  type: string;
  fast: boolean;
}

/**
 * Reads `argv` into two slots: `slotFlags` maps a flag to slot 1 or 2, and a
 * bare argument fills the first empty slot. `-t`/`--type` and `--fast` apply
 * only when `docs` is true.
 */
function readArgs(
  argv: readonly string[],
  slotFlags: Record<string, 1 | 2>,
  docs: boolean,
): Parsed {
  const parsed: Parsed = { first: "", second: "", type: "json", fast: false };
  for (let at = 0; at < argv.length; at += 1) {
    const arg = argv[at] ?? "";
    // Own keys only: a library named "constructor" is not a flag.
    const slot = Object.hasOwn(slotFlags, arg) ? slotFlags[arg] : undefined;
    if (slot !== undefined) {
      const value = flagValue(TOOL, argv, at);
      if (slot === 1) parsed.first = value;
      else parsed.second = value;
      at += 1;
    } else if (docs && (arg === "-t" || arg === "--type")) {
      parsed.type = flagValue(TOOL, argv, at);
      at += 1;
    } else if (docs && arg === "--fast") {
      parsed.fast = true;
    } else if (parsed.first === "") {
      parsed.first = arg;
    } else if (parsed.second === "") {
      parsed.second = arg;
    } else {
      throw new CliExit(1, `Unknown option: ${arg}`);
    }
  }
  return parsed;
}

export function searchCall(argv: readonly string[]): Context7Call {
  const { first: library, second: query } = readArgs(
    argv,
    { "-l": 1, "--library": 1, "-q": 2, "--query": 2 },
    false,
  );
  if (library === "") throw new CliExit(1, SEARCH_USAGE);
  return {
    endpoint: "libs/search",
    params: [
      ["libraryName", library],
      ["query", query === "" ? library : query],
    ],
    json: true,
  };
}

export function docsCall(argv: readonly string[]): Context7Call {
  const parsed = readArgs(argv, { "-l": 1, "--library-id": 1, "-q": 2, "--query": 2 }, true);
  if (parsed.first === "" || parsed.second === "") throw new CliExit(1, DOCS_USAGE);
  const params: Array<readonly [string, string]> = [
    ["libraryId", parsed.first],
    ["query", parsed.second],
    ["type", parsed.type],
  ];
  if (parsed.fast) params.push(["fast", "true"]);
  return { endpoint: "context", params, json: parsed.type === "json" };
}
