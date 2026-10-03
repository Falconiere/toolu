#!/usr/bin/env -S bun --no-env-file
// @bun

// packages/toolu-core/src/cli/cli.ts
class CliExit extends Error {
  code;
  stdout;
  constructor(code, message = "", stdout = "") {
    super(message);
    this.name = "CliExit";
    this.code = code;
    this.stdout = stdout;
  }
}
function flagValue(tool, argv, index) {
  const value = argv[index + 1];
  if (value === undefined)
    throw new CliExit(1, `${tool}: ${argv[index] ?? ""} needs a value`);
  return value;
}
var JQ_NUMBER = /^[ \t\n\r]*[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?[ \t\n\r]*$/;
function numberValue(tool, flag, text) {
  const value = Number(text);
  if (!JQ_NUMBER.test(text) || !Number.isFinite(value)) {
    throw new CliExit(2, `${tool}: ${flag} must be a number`);
  }
  return value;
}
function isBrokenPipe(error) {
  return error instanceof Error && "code" in error && error.code === "EPIPE";
}
async function writeStdout(text) {
  if (text.length === 0)
    return;
  try {
    await Bun.write(Bun.stdout, text);
  } catch (error) {
    if (isBrokenPipe(error))
      throw new CliExit(141);
    throw error;
  }
}
async function writeStderr(text) {
  if (text !== "")
    await Bun.write(Bun.stderr, text.endsWith(`
`) ? text : `${text}
`);
}
async function report(exit) {
  let code = exit.code;
  try {
    await writeStdout(exit.stdout);
  } catch (error) {
    if (!(error instanceof CliExit))
      throw error;
    code = error.code;
  }
  await writeStderr(exit.message);
  return code;
}
async function runCli(main) {
  let code;
  try {
    code = await main();
  } catch (error) {
    if (error instanceof CliExit) {
      code = await report(error);
    } else {
      await writeStderr(error instanceof Error ? error.message : String(error));
      code = 1;
    }
  }
  process.exit(code);
}

// packages/toolu-core/src/rest/rest.ts
function formatJson(value) {
  return `${JSON.stringify(value, null, 2)}
`;
}
function tryParse(text) {
  try {
    const value = JSON.parse(text);
    return { ok: true, value };
  } catch {
    return { ok: false };
  }
}
function parseJson(tool, text) {
  const parsed = tryParse(text);
  if (!parsed.ok)
    throw new CliExit(5, `${tool}: response is not JSON`);
  return parsed.value;
}
function jsonOutput(tool, text, project = (value) => value) {
  if (text.trim() === "")
    return "";
  return formatJson(project(parseJson(tool, text)));
}
function reason(error) {
  return error instanceof Error ? error.message : String(error);
}
async function exchange(tool, url, init) {
  try {
    return await fetch(url, { ...init, redirect: "manual" });
  } catch (error) {
    throw new CliExit(1, `${tool}: request failed: ${reason(error)}`);
  }
}
async function readBody(tool, read) {
  try {
    return await read();
  } catch (error) {
    throw new CliExit(1, `${tool}: request failed: ${reason(error)}`);
  }
}
async function send(tool, request) {
  const init = { method: request.method ?? "GET", headers: { ...request.headers } };
  if (request.payload !== undefined)
    init.body = request.payload;
  else if (request.body !== undefined)
    init.body = JSON.stringify(request.body);
  const response = await exchange(tool, request.url, init);
  const { status } = response;
  const text = await readBody(tool, () => response.text());
  if (status >= 400) {
    const parsed = request.json === false ? undefined : tryParse(text);
    const body = parsed?.ok === true ? formatJson(parsed.value) : text;
    throw new CliExit(22, `${tool}: HTTP ${status} from ${request.url}`, body);
  }
  return text;
}
var REDIRECTS = new Set([301, 302, 303, 307, 308]);

// plugins/exa-search/hooks/src/exa/lean.ts
var RESULT_FIELDS = ["title", "url", "publishedDate", "author", "highlights", "text", "summary"];
function isRecord(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
function leanResult(result) {
  const source = isRecord(result) ? result : {};
  const kept = {};
  for (const field of RESULT_FIELDS) {
    const value = source[field];
    if (value !== undefined && value !== null && value !== "")
      kept[field] = value;
  }
  return kept;
}
function leanResponse(response) {
  const source = isRecord(response) ? response : {};
  const results = Array.isArray(source["results"]) ? source["results"] : [];
  return { requestId: source["requestId"] ?? null, results: results.map(leanResult) };
}

// plugins/exa-search/hooks/src/exa/usage.ts
var MAIN_USAGE = `Exa Search CLI

Usage: search.sh <command> [options]

Environment:
  EXA_API_KEY  Required. Exa API key.

Commands:
  search   Search the web (default if no command given)
  crawl    Extract content from URLs
  similar  Find pages similar to a URL

Run 'search.sh <command>' with no args for command-specific help.`;
var SEARCH_USAGE = `Usage: search.sh search -q <query> [options]
  -n, --num-results  Number of results (default: 10)
  -t, --type         instant|fast|auto|deep-lite|deep|deep-reasoning (default: auto)
  -c, --category     company|research paper|news|personal site|financial report|people
  --include-domains  Comma-separated domains to include
  --exclude-domains  Comma-separated domains to exclude
  --start-date       Start published date (YYYY-MM-DD)
  --end-date         End published date (YYYY-MM-DD)
  --include-text     Text that must appear in results
  --exclude-text     Text to exclude from results
  --highlights N     Max highlight chars (default: 4000)
  --with-text        Include full text in results
  --lean             Strip image/favicon/subpages/entities for AI prompts`;
var CRAWL_USAGE = `Usage: search.sh crawl <url> [url...] [-m max_chars]
  Extracts content from one or more URLs
  -m, --max-chars  Max characters per page (default: 3000)`;
var SIMILAR_USAGE = `Usage: search.sh similar <url> [-n num_results]
  Finds pages similar to the given URL
  -n, --num-results  Number of results (default: 10)
  --highlights N     Max highlight chars (default: 4000)`;

// plugins/exa-search/hooks/src/exa/requests.ts
var TOOL = "exa-search";
var SEARCH_VALUES = {
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
  "--highlights": "highlights"
};
function readSearchArgs(argv) {
  const args = { values: new Map, spelled: new Map, switches: new Set };
  for (let at = 0;at < argv.length; at += 1) {
    const arg = argv[at] ?? "";
    const option = Object.hasOwn(SEARCH_VALUES, arg) ? SEARCH_VALUES[arg] : undefined;
    if (option !== undefined) {
      args.values.set(option, flagValue(TOOL, argv, at));
      args.spelled.set(option, arg);
      at += 1;
    } else if (arg === "--with-text" || arg === "--lean") {
      args.switches.add(arg);
    } else if ((args.values.get("query") ?? "") === "") {
      args.values.set("query", arg);
    } else {
      throw new CliExit(1, `Unknown option: ${arg}`);
    }
  }
  return args;
}
function numberOption(args, option, fallback) {
  const text = args.values.get(option);
  return text === undefined ? fallback : numberValue(TOOL, args.spelled.get(option) ?? option, text);
}
function searchCall(argv) {
  const args = readSearchArgs(argv);
  const { values } = args;
  const query = values.get("query") ?? "";
  if (query === "")
    throw new CliExit(1, SEARCH_USAGE);
  const contents = {
    highlights: { maxCharacters: numberOption(args, "highlights", 4000) }
  };
  if (args.switches.has("--with-text"))
    contents["text"] = true;
  const body = {
    query,
    type: values.get("type") ?? "auto",
    numResults: numberOption(args, "numResults", 10),
    contents
  };
  const set = (option, apply) => {
    const value = values.get(option);
    if (value !== undefined && value !== "")
      apply(value);
  };
  set("category", (value) => body["category"] = value);
  set("includeDomains", (value) => body["includeDomains"] = value.split(","));
  set("excludeDomains", (value) => body["excludeDomains"] = value.split(","));
  set("startDate", (value) => body["startPublishedDate"] = `${value}T00:00:00.000Z`);
  set("endDate", (value) => body["endPublishedDate"] = `${value}T00:00:00.000Z`);
  set("includeText", (value) => body["includeText"] = [value]);
  set("excludeText", (value) => body["excludeText"] = [value]);
  return { endpoint: "search", body, lean: args.switches.has("--lean") };
}
function crawlCall(argv) {
  let maxChars = "3000";
  let maxFlag = "-m";
  const urls = [];
  for (let at = 0;at < argv.length; at += 1) {
    const arg = argv[at] ?? "";
    if (arg === "-m" || arg === "--max-chars") {
      maxChars = flagValue(TOOL, argv, at);
      maxFlag = arg;
      at += 1;
    } else {
      urls.push(arg);
    }
  }
  if (urls.length === 0)
    throw new CliExit(1, CRAWL_USAGE);
  const maxCharacters = numberValue(TOOL, maxFlag, maxChars);
  return {
    endpoint: "contents",
    body: { urls, text: true, highlights: { maxCharacters } },
    lean: false
  };
}
function similarCall(argv) {
  const raw = {
    url: "",
    numResults: "10",
    highlights: "4000",
    numFlag: "-n",
    highlightsFlag: "--highlights"
  };
  for (let at = 0;at < argv.length; at += 1) {
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
  if (raw.url === "")
    throw new CliExit(1, SIMILAR_USAGE);
  const body = {
    url: raw.url,
    numResults: numberValue(TOOL, raw.numFlag, raw.numResults),
    contents: {
      highlights: { maxCharacters: numberValue(TOOL, raw.highlightsFlag, raw.highlights) }
    }
  };
  return { endpoint: "findSimilar", body, lean: false };
}

// plugins/exa-search/hooks/src/search.ts
var TOOL2 = "exa-search";
var EXA_URL = "https://api.exa.ai";
function callFor(argv) {
  const [command = "", ...rest] = argv;
  switch (command) {
    case "":
    case "-h":
    case "--help":
      throw new CliExit(1, MAIN_USAGE);
    case "search":
      return searchCall(rest);
    case "crawl":
      return crawlCall(rest);
    case "similar":
      return similarCall(rest);
    default:
      return searchCall(argv);
  }
}
async function main() {
  const key = process.env["EXA_API_KEY"] ?? "";
  if (key === "")
    throw new CliExit(1, `${TOOL2}: EXA_API_KEY unset`);
  const call = callFor(process.argv.slice(2));
  const text = await send(TOOL2, {
    url: `${EXA_URL}/${call.endpoint}`,
    method: "POST",
    headers: { "x-api-key": key, "Content-Type": "application/json", Accept: "application/json" },
    body: call.body
  });
  await writeStdout(call.lean ? jsonOutput(TOOL2, text, leanResponse) : jsonOutput(TOOL2, text));
  return 0;
}
await runCli(main);
