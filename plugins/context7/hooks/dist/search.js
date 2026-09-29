#!/usr/bin/env bun
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
function isBrokenPipe(error) {
  return error instanceof Error && "code" in error && error.code === "EPIPE";
}
async function writeStdout(text) {
  if (text === "")
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
async function send(tool, request) {
  const init = {
    method: request.method ?? "GET",
    headers: { ...request.headers },
    redirect: "manual"
  };
  if (request.body !== undefined)
    init.body = JSON.stringify(request.body);
  let status;
  let text;
  try {
    const response = await fetch(request.url, init);
    status = response.status;
    text = await response.text();
  } catch (error) {
    throw new CliExit(1, `${tool}: request failed: ${reason(error)}`);
  }
  if (status >= 400) {
    const parsed = request.json === false ? undefined : tryParse(text);
    const body = parsed?.ok === true ? formatJson(parsed.value) : text;
    throw new CliExit(22, `${tool}: HTTP ${status} from ${request.url}`, body);
  }
  return text;
}
function quote(value) {
  return encodeURIComponent(value).replaceAll(/[!'()*]/g, (char) => `%${char.charCodeAt(0).toString(16).toUpperCase()}`);
}
function encodeQuery(params) {
  if (params.length === 0)
    return "";
  return `?${params.map(([key, value]) => `${key}=${quote(value)}`).join("&")}`;
}

// plugins/context7/hooks/src/context7/usage.ts
var MAIN_USAGE = `Context7 CLI \u2014 Library Documentation Lookup

Usage: search.sh <command> [options]

Environment:
  CONTEXT7_API_KEY  Optional. If set and starts with 'ctx7sk', sent as Bearer token.

Commands:
  search  Find libraries by name (resolve library ID)
  docs    Query documentation for a library

Workflow:
  1. search.sh search <library>    # find the library ID
  2. search.sh docs <id> <query>   # query its docs

Run 'search.sh <command>' with no args for command-specific help.`;
var SEARCH_USAGE = `Usage: search.sh search <library> [query]
  Searches for libraries matching the name

  -l, --library  Library name (required)
  -q, --query    Context for ranking results

Examples:
  search.sh search react
  search.sh search tokio "async runtime for Rust"`;
var DOCS_USAGE = `Usage: search.sh docs <library_id> <query>
  Retrieves documentation context for a library

  -l, --library-id  Context7 library ID, e.g. /vercel/next.js (required)
  -q, --query       Your question (required)
  -t, --type        Output format: json|txt (default: json; txt is LLM-prompt-ready)
  --fast            Skip LLM reranking, return top vector-search hits (lower latency)

Examples:
  search.sh docs /vercel/next.js "app router file conventions"
  search.sh docs /tokio-rs/tokio "spawn async tasks" -t txt

Tip: Run 'search.sh search <name>' first to find the library ID.`;

// plugins/context7/hooks/src/context7/requests.ts
var TOOL = "context7";
function readArgs(argv, slotFlags, docs) {
  const parsed = { first: "", second: "", type: "json", fast: false };
  for (let at = 0;at < argv.length; at += 1) {
    const arg = argv[at] ?? "";
    const slot = Object.hasOwn(slotFlags, arg) ? slotFlags[arg] : undefined;
    if (slot !== undefined) {
      const value = flagValue(TOOL, argv, at);
      if (slot === 1)
        parsed.first = value;
      else
        parsed.second = value;
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
function searchCall(argv) {
  const { first: library, second: query } = readArgs(argv, { "-l": 1, "--library": 1, "-q": 2, "--query": 2 }, false);
  if (library === "")
    throw new CliExit(1, SEARCH_USAGE);
  return {
    endpoint: "libs/search",
    params: [
      ["libraryName", library],
      ["query", query === "" ? library : query]
    ],
    json: true
  };
}
function docsCall(argv) {
  const parsed = readArgs(argv, { "-l": 1, "--library-id": 1, "-q": 2, "--query": 2 }, true);
  if (parsed.first === "" || parsed.second === "")
    throw new CliExit(1, DOCS_USAGE);
  const params = [
    ["libraryId", parsed.first],
    ["query", parsed.second],
    ["type", parsed.type]
  ];
  if (parsed.fast)
    params.push(["fast", "true"]);
  return { endpoint: "context", params, json: parsed.type === "json" };
}

// plugins/context7/hooks/src/search.ts
var TOOL2 = "context7";
var C7_URL = "https://context7.com/api/v2";
function callFor(argv) {
  const [command = "", ...rest] = argv;
  switch (command) {
    case "":
    case "-h":
    case "--help":
      throw new CliExit(1, MAIN_USAGE);
    case "search":
      return searchCall(rest);
    case "docs":
      return docsCall(rest);
    default:
      return searchCall(argv);
  }
}
async function main() {
  const call = callFor(process.argv.slice(2));
  const headers = { Accept: "application/json" };
  const key = process.env["CONTEXT7_API_KEY"] ?? "";
  if (key.startsWith("ctx7sk"))
    headers["Authorization"] = `Bearer ${key}`;
  const text = await send(TOOL2, {
    url: `${C7_URL}/${call.endpoint}${encodeQuery(call.params)}`,
    headers,
    json: call.json
  });
  await writeStdout(call.json ? jsonOutput(TOOL2, text) : text);
  return 0;
}
await runCli(main);
