/** argv → Exa request bodies, with the bash search.sh's parsing rules and field order. */
import { CliExit, flagValue, numberValue } from "@toolu/core/cli";
import { CRAWL_USAGE, SEARCH_USAGE, SIMILAR_USAGE } from "./usage.ts";

const TOOL = "exa-search";

export interface ExaCall {
  readonly endpoint: "search" | "contents" | "findSimilar";
  readonly body: Record<string, unknown>;
  readonly lean: boolean;
}

/** Search flags that take a value, keyed by every spelling, to the option they set. */
const SEARCH_VALUES: Readonly<Record<string, string>> = {
  "-q": "query",
  "--query": "query",
  "-n": "numResults",
  "--num-results": "numResults",
  "-t": "type",
  "--type": "type",
  "-c": "category",
  "--category": "category",
  "--include-domains": "includeDomains",
  "--exclude-domains": "excludeDomains",
  "--start-date": "startDate",
  "--end-date": "endDate",
  "--include-text": "includeText",
  "--exclude-text": "excludeText",
  "--highlights": "highlights",
};

interface SearchArgs {
  /** Option name → raw value. */
  readonly values: Map<string, string>;
  /** Option name → the flag spelling the caller used, for error messages. */
  readonly spelled: Map<string, string>;
  readonly switches: Set<string>;
}

function readSearchArgs(argv: readonly string[]): SearchArgs {
  const args: SearchArgs = { values: new Map(), spelled: new Map(), switches: new Set() };
  for (let at = 0; at < argv.length; at += 1) {
    const arg = argv[at] ?? "";
    // Own keys only: a bare query such as "constructor" is not a flag.
    const option = Object.hasOwn(SEARCH_VALUES, arg) ? SEARCH_VALUES[arg] : undefined;
    if (option !== undefined) {
      args.values.set(option, flagValue(TOOL, argv, at));
      args.spelled.set(option, arg);
      at += 1;
    } else if (arg === "--with-text" || arg === "--lean") {
      args.switches.add(arg);
    } else if ((args.values.get("query") ?? "") === "") {
      // A bare argument is the query, as in bash.
      args.values.set("query", arg);
    } else {
      throw new CliExit(1, `Unknown option: ${arg}`);
    }
  }
  return args;
}

function numberOption(args: SearchArgs, option: string, fallback: number): number {
  const text = args.values.get(option);
  return text === undefined
    ? fallback
    : numberValue(TOOL, args.spelled.get(option) ?? option, text);
}

export function searchCall(argv: readonly string[]): ExaCall {
  const args = readSearchArgs(argv);
  const { values } = args;
  const query = values.get("query") ?? "";
  if (query === "") throw new CliExit(1, SEARCH_USAGE);
  const contents: Record<string, unknown> = {
    highlights: { maxCharacters: numberOption(args, "highlights", 4000) },
  };
  if (args.switches.has("--with-text")) contents["text"] = true;
  const body: Record<string, unknown> = {
    query,
    type: values.get("type") ?? "auto",
    numResults: numberOption(args, "numResults", 10),
    contents,
  };
  const set = (option: string, apply: (value: string) => void): void => {
    const value = values.get(option);
    if (value !== undefined && value !== "") apply(value);
  };
  set("category", (value) => (body["category"] = value));
  set("includeDomains", (value) => (body["includeDomains"] = value.split(",")));
  set("excludeDomains", (value) => (body["excludeDomains"] = value.split(",")));
  set("startDate", (value) => (body["startPublishedDate"] = `${value}T00:00:00.000Z`));
  set("endDate", (value) => (body["endPublishedDate"] = `${value}T00:00:00.000Z`));
  set("includeText", (value) => (body["includeText"] = [value]));
  set("excludeText", (value) => (body["excludeText"] = [value]));
  return { endpoint: "search", body, lean: args.switches.has("--lean") };
}

export function crawlCall(argv: readonly string[]): ExaCall {
  let maxChars = "3000";
  let maxFlag = "-m";
  const urls: string[] = [];
  for (let at = 0; at < argv.length; at += 1) {
    const arg = argv[at] ?? "";
    if (arg === "-m" || arg === "--max-chars") {
      maxChars = flagValue(TOOL, argv, at);
      maxFlag = arg;
      at += 1;
    } else {
      urls.push(arg);
    }
  }
  if (urls.length === 0) throw new CliExit(1, CRAWL_USAGE);
  const maxCharacters = numberValue(TOOL, maxFlag, maxChars);
  return {
    endpoint: "contents",
    body: { urls, text: true, highlights: { maxCharacters } },
    lean: false,
  };
}

export function similarCall(argv: readonly string[]): ExaCall {
  const raw = {
    url: "",
    numResults: "10",
    highlights: "4000",
    numFlag: "-n",
    highlightsFlag: "--highlights",
  };
  for (let at = 0; at < argv.length; at += 1) {
    const arg = argv[at] ?? "";
    if (arg === "-n" || arg === "--num-results") {
      raw.numResults = flagValue(TOOL, argv, at);
      raw.numFlag = arg;
      at += 1;
    } else if (arg === "--highlights") {
      raw.highlights = flagValue(TOOL, argv, at);
      at += 1;
    } else if (raw.url === "") {
      raw.url = arg;
    } else {
      throw new CliExit(1, `Unknown option: ${arg}`);
    }
  }
  if (raw.url === "") throw new CliExit(1, SIMILAR_USAGE);
  const body = {
    url: raw.url,
    numResults: numberValue(TOOL, raw.numFlag, raw.numResults),
    contents: {
      highlights: { maxCharacters: numberValue(TOOL, raw.highlightsFlag, raw.highlights) },
    },
  };
  return { endpoint: "findSimilar", body, lean: false };
}
