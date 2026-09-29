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
var MAX_REDIRECTS = 50;
function withoutAuthorization(headers) {
  return Object.fromEntries(Object.entries(headers).filter(([name]) => name.toLowerCase() !== "authorization"));
}
async function follow(tool, hop) {
  const response = await exchange(tool, hop.url, { method: "GET", headers: hop.headers });
  const location = response.headers.get("location");
  if (REDIRECTS.has(response.status) && location !== null) {
    await response.body?.cancel();
    if (hop.count >= MAX_REDIRECTS)
      throw new CliExit(47, `${tool}: too many redirects from ${hop.url}`);
    const url = new URL(location, hop.url).href;
    const headers = new URL(url).origin === hop.origin ? hop.headers : withoutAuthorization(hop.headers);
    return follow(tool, { ...hop, url, headers, count: hop.count + 1 });
  }
  const bytes = new Uint8Array(await readBody(tool, () => response.arrayBuffer()));
  if (response.status >= 400) {
    const body = new TextDecoder().decode(bytes);
    throw new CliExit(22, `${tool}: HTTP ${response.status} from ${hop.url}`, body);
  }
  return bytes;
}
function download(tool, request) {
  const origin = new URL(request.url).origin;
  return follow(tool, { url: request.url, headers: { ...request.headers }, origin, count: 0 });
}
function quote(value) {
  return encodeURIComponent(value).replaceAll(/[!'()*]/g, (char) => `%${char.charCodeAt(0).toString(16).toUpperCase()}`);
}
function encodeQuery(params) {
  if (params.length === 0)
    return "";
  return `?${params.map(([key, value]) => `${key}=${quote(value)}`).join("&")}`;
}

// plugins/jira/hooks/src/jira/attachment.ts
import { randomBytes } from "crypto";
import { statSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { basename, join as join2 } from "path";

// plugins/jira/hooks/src/jira/flags.ts
function lookup(table, arg) {
  return table !== undefined && Object.hasOwn(table, arg) ? table[arg] : undefined;
}
function unknownOption(context, arg) {
  throw new CliExit(1, `${context}: unknown option '${arg}'`);
}
function readFlags(argv, spec, other) {
  const flags = { values: new Map, lists: new Map, switches: new Set };
  for (let at = 0;at < argv.length; at += 1) {
    const arg = argv[at] ?? "";
    const value = lookup(spec.values, arg);
    const list = lookup(spec.lists, arg);
    const toggle = lookup(spec.switches, arg);
    if (value !== undefined) {
      flags.values.set(value, flagValue("jira", argv, at));
      at += 1;
    } else if (list !== undefined) {
      flags.lists.set(list, [...flags.lists.get(list) ?? [], flagValue("jira", argv, at)]);
      at += 1;
    } else if (toggle !== undefined) {
      flags.switches.add(toggle);
    } else {
      other(arg, flags);
    }
  }
  return flags;
}
function onlyFlags(context, argv, spec) {
  return readFlags(argv, spec, (arg) => unknownOption(context, arg));
}
function shiftArg(argv) {
  const [first = "", ...rest] = argv;
  return [first, rest];
}
function route(usage, actions, ctx, argv) {
  const [action, rest] = shiftArg(argv);
  const run = Object.hasOwn(actions, action) ? actions[action] : undefined;
  if (run === undefined)
    throw new CliExit(1, usage);
  return run(ctx, rest);
}

// plugins/jira/hooks/src/jira/creds.ts
import { spawnSync } from "child_process";
import { readFileSync } from "fs";
import { join } from "path";
function readable(path) {
  try {
    return readFileSync(path, "utf8");
  } catch {
    return;
  }
}
function yamlGet(content, key) {
  const line = content.split(`
`).find((candidate) => candidate.startsWith(`${key}:`));
  return line === undefined ? "" : line.replace(/^[^:]*:\s*/, "").replace(/\s+$/, "");
}
function netrcToken(env, host) {
  const content = readable(env["NETRC"] || join(env["HOME"] ?? "", ".netrc"));
  if (content === undefined)
    return "";
  const tokens = content.split(/\s+/).filter((token) => token !== "");
  let inHost = false;
  for (let at = 0;at < tokens.length; at += 1) {
    if (tokens[at] === "machine")
      inHost = tokens[at + 1] === host;
    if (inHost && tokens[at] === "password")
      return tokens[at + 1] ?? "";
  }
  return "";
}
function helperOutput(env, command, args) {
  const run = spawnSync(command, args, {
    env,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"]
  });
  return run.error === undefined ? run.stdout.replace(/\n+$/, "") : "";
}
function keyringToken(env, login) {
  const service = env["JIRA_KEYRING_SERVICE"] || "jira-cli";
  const token = helperOutput(env, "security", [
    "find-generic-password",
    "-s",
    service,
    "-a",
    login,
    "-w"
  ]);
  if (token !== "")
    return token;
  return helperOutput(env, "secret-tool", ["lookup", "service", service, "username", login]);
}
function configPath(env) {
  const config = env["XDG_CONFIG_HOME"] || join(env["HOME"] ?? "", ".config");
  return env["JIRA_CLI_CONFIG"] || join(config, ".jira", ".config.yml");
}
function tokenFor(env, content, server, login) {
  let token = yamlGet(content, "api_token");
  if (token === "" && server !== "") {
    const host = server.replace(/^.*?:\/\//, "").replace(/\/.*$/, "");
    token = netrcToken(env, host);
  }
  if (token === "" && login !== "")
    token = keyringToken(env, login);
  return token;
}
function withCliCreds(env) {
  const content = readable(configPath(env));
  if (content === undefined)
    return env;
  const filled = { ...env };
  const server = yamlGet(content, "server");
  const login = yamlGet(content, "login");
  const installation = yamlGet(content, "installation");
  if (!filled["JIRA_BASE_URL"] && server !== "")
    filled["JIRA_BASE_URL"] = server;
  if (!filled["JIRA_EMAIL"] && login !== "")
    filled["JIRA_EMAIL"] = login;
  if (!filled["JIRA_API_VERSION"] && installation !== "") {
    filled["JIRA_API_VERSION"] = /^[Cc]loud$/.test(installation) ? "3" : "2";
  }
  if (!filled["JIRA_PAT"] && !filled["JIRA_API_TOKEN"]) {
    const token = tokenFor(filled, content, server, login);
    if (token !== "")
      filled["JIRA_API_TOKEN"] = token;
  }
  return filled;
}

// plugins/jira/hooks/src/jira/usage.ts
var USAGE = `jira \u2014 Jira from the session (Cloud + Server/DC)

Usage: jira [--api-version N] [--lean] <family> <action> [options]

Families:
  search       JQL search
  issue        get|create|update|delete|comment|transition|transitions|assign
  board        list|get|issues
  sprint       list|get|create|issues|move|start|complete
  worklog      add|list|delete
  project      list|get|versions|components
  user         whoami|search|get
  attachment   add|list|get
  raw          <METHOD> <path> [body]
  plan         init|run|status|path

Environment:
  JIRA_BASE_URL                      required (e.g. https://acme.atlassian.net)
  JIRA_PAT                           Bearer auth (Cloud or Server/DC)
  JIRA_EMAIL + JIRA_API_TOKEN        basic auth (Cloud API token)
  JIRA_API_VERSION                   2 or 3 (default 3)
`;
var CREDS_HELP = `jira: no Jira credentials found yet \u2014 let's get you connected.

This plugin reuses your \`jira\` CLI login automatically when it's configured,
or reads JIRA_* environment variables. Right now neither is set.

Connect with either:

  \u2022 Your jira CLI (easiest if it's installed):
      jira init
    then re-run your command \u2014 the plugin picks up the server, login, and
    API token from the CLI automatically.

  \u2022 Or environment variables:
      export JIRA_BASE_URL=https://your-site.atlassian.net
      export JIRA_EMAIL=you@example.com
      export JIRA_API_TOKEN=\u2026        # id.atlassian.com \u2192 Security \u2192 API tokens
    (Jira Server/Data Center: use JIRA_PAT instead of JIRA_EMAIL + JIRA_API_TOKEN.)

Nothing is broken \u2014 this is just a one-time setup step.
`;
var PLAN_USAGE = `jira plan \u2014 decompose ticket work into verifiable steps the dashboard renders

  plan init <KEY>                                 scaffold the plan doc for a ticket
  plan run <DOC> [--step <id>] [--activity <s>]   run checks, write the ledger
  plan status <KEY>                               print the ledger summary
  plan path <KEY>                                 print the ledger path

Every step's \`check\` must be a live Jira assertion that exits 0 only when Jira
itself reflects the change.
`;

// plugins/jira/hooks/src/jira/http.ts
var TOOL = "jira";
function authorization(env) {
  if (env["JIRA_PAT"])
    return `Bearer ${env["JIRA_PAT"]}`;
  const email = env["JIRA_EMAIL"];
  const token = env["JIRA_API_TOKEN"];
  if (!email || !token)
    return;
  return `Basic ${Buffer.from(`${email}:${token}`).toString("base64")}`;
}
function connect(processEnv, globals) {
  const env = withCliCreds(processEnv);
  const base = (env["JIRA_BASE_URL"] ?? "").replace(/\/+$/, "");
  const auth = authorization(env);
  if (!env["JIRA_BASE_URL"] || auth === undefined)
    throw new CliExit(1, CREDS_HELP);
  const version = globals.version || env["_JIRA_VER"] || env["JIRA_API_VERSION"] || "3";
  if (version !== "2" && version !== "3") {
    throw new CliExit(1, `jira: api version must be 2 or 3 (got '${version}')`);
  }
  const lean = globals.lean || env["_JIRA_LEAN"] === "1";
  return { base, authorization: auth, version, lean, env };
}
function api(conn) {
  return `/rest/api/${conn.version}`;
}
function jiraRequest(conn, method, path, body) {
  const headers = {
    Accept: "application/json",
    Authorization: conn.authorization
  };
  const request = { url: `${conn.base}${path}`, method, headers };
  if (body === undefined)
    return request;
  headers["Content-Type"] = "application/json";
  return typeof body === "string" ? { ...request, payload: body } : { ...request, body };
}
function call(conn, method, path, body) {
  return send(TOOL, jiraRequest(conn, method, path, body));
}
async function lookup2(conn, path, options = {}) {
  let text;
  try {
    text = await call(conn, options.method ?? "GET", path, options.body);
  } catch (error) {
    if (error instanceof CliExit && error.code === 22) {
      throw new CliExit(options.code ?? 1, error.message);
    }
    throw error;
  }
  return parseJson(TOOL, text);
}
async function printLean(conn, text, projection = (value) => value) {
  await writeStdout(conn.lean ? jsonOutput(TOOL, text, projection) : jsonOutput(TOOL, text));
}
async function mutate(conn, method, path, body, done) {
  const text = await send(TOOL, { ...jiraRequest(conn, method, path, body), json: false });
  await writeStdout(`${text}${done}
`);
  return 0;
}

// plugins/jira/hooks/src/jira/jq.ts
function isObject(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
function typeName(value) {
  if (value === null || value === undefined)
    return "null";
  if (Array.isArray(value))
    return "array";
  return typeof value;
}
function get(value, ...keys) {
  let current = value;
  for (const key of keys) {
    if (current === null || current === undefined)
      return null;
    if (!isObject(current)) {
      throw new CliExit(5, `jq: error: Cannot index ${typeName(current)} with "${key}"`);
    }
    current = Object.hasOwn(current, key) ? current[key] : null;
  }
  return current ?? null;
}
function iterate(value) {
  if (Array.isArray(value))
    return value;
  if (isObject(value))
    return Object.values(value);
  throw new CliExit(5, `jq: error: Cannot iterate over ${typeName(value)}`);
}
function rows(value, key, row) {
  return iterate(get(value, key)).map(row);
}
function pick(value, ...keys) {
  return Object.fromEntries(keys.map((key) => [key, get(value, key)]));
}
function alt(value, fallback) {
  return value === null || value === undefined || value === false ? fallback : value;
}
function text(value) {
  return typeof value === "string" ? value : JSON.stringify(value ?? null);
}

// plugins/jira/hooks/src/jira/attachment.ts
var USAGE2 = "Usage: jira attachment <add|list|get|download|read> ...";
function isFile(path) {
  return statSync(path, { throwIfNoEntry: false })?.isFile() === true;
}
async function add(conn, [key = "", file = ""]) {
  if (key === "" || file === "")
    throw new CliExit(1, "Usage: jira attachment add <KEY> <FILE>");
  if (!isFile(file))
    throw new CliExit(1, `attachment add: file not found: ${file}`);
  const form = new FormData;
  form.append("file", Bun.file(file), basename(file));
  const request = jiraRequest(conn, "POST", `${api(conn)}/issue/${key}/attachments`);
  const headers = { ...request.headers, "X-Atlassian-Token": "no-check" };
  await printLean(conn, await send(TOOL, { ...request, headers, payload: form }));
  return 0;
}
async function list(conn, [key = ""]) {
  if (key === "")
    throw new CliExit(1, "Usage: jira attachment list <KEY>");
  const body = await call(conn, "GET", `${api(conn)}/issue/${key}?fields=attachment`);
  await printLean(conn, body, (value) => ({
    attachments: rows(get(value, "fields"), "attachment", (item) => pick(item, "id", "filename", "size"))
  }));
  return 0;
}
async function getAttachment(conn, [id = ""]) {
  if (id === "")
    throw new CliExit(1, "Usage: jira attachment get <ATTACHMENT_ID>");
  await printLean(conn, await call(conn, "GET", `${api(conn)}/attachment/${id}`));
  return 0;
}
function content(conn, id) {
  const request = jiraRequest(conn, "GET", `${api(conn)}/attachment/content/${id}`);
  return download(TOOL, { ...request, headers: { ...request.headers, Accept: "*/*" } });
}
async function save(conn, argv) {
  const [id, rest] = shiftArg(argv);
  const flags = onlyFlags("attachment download", rest, {
    values: { "-o": "out", "--output": "out" }
  });
  if (id === "") {
    throw new CliExit(1, "Usage: jira attachment download <ATTACHMENT_ID> [-o OUTFILE]");
  }
  let out = flags.values.get("out") ?? "";
  if (out === "") {
    const meta = await lookup2(conn, `${api(conn)}/attachment/${id}`, { code: 22 });
    const name = text(alt(get(meta, "filename"), ""));
    out = name === "" ? `attachment-${id}` : name;
  }
  writeFileSync(out, await content(conn, id));
  await writeStdout(`downloaded attachment ${id} -> ${out}
`);
  return 0;
}
var TEXT_TYPES = new Set([
  "application/json",
  "application/xml",
  "application/javascript",
  "application/x-yaml",
  "application/yaml"
]);
function isTextLike(mime) {
  return mime.startsWith("text/") || TEXT_TYPES.has(mime) || mime.endsWith("+json") || mime.endsWith("+xml");
}
function tempFile(bytes) {
  const path = join2(tmpdir(), `jira-attachment.${randomBytes(6).toString("hex")}`);
  writeFileSync(path, bytes, { flag: "wx" });
  return path;
}
async function read(conn, [id = ""]) {
  if (id === "")
    throw new CliExit(1, "Usage: jira attachment read <ATTACHMENT_ID>");
  const meta = await lookup2(conn, `${api(conn)}/attachment/${id}`);
  const mime = text(alt(get(meta, "mimeType"), ""));
  const name = text(alt(get(meta, "filename"), "attachment"));
  let bytes;
  try {
    bytes = await content(conn, id);
  } catch (error) {
    if (error instanceof CliExit && error.code === 22)
      throw new CliExit(1, error.message);
    throw error;
  }
  if (isTextLike(mime)) {
    await writeStdout(`# ${name} (${mime})

`);
    await writeStdout(bytes);
    return 0;
  }
  const saved = tempFile(bytes);
  await writeStdout(`${name} \u2014 binary attachment (${mime || "unknown"}, ${bytes.length} bytes) saved to ${saved}
` + `Run \`jira attachment download ${id} -o <path>\` to save it elsewhere.
`);
  return 0;
}
var ACTIONS = {
  add,
  list,
  get: getAttachment,
  download: save,
  read
};
function attachment(conn, argv) {
  return route(USAGE2, ACTIONS, conn, argv);
}

// plugins/jira/hooks/src/jira/paginate.ts
var MAX_PAGES = 1000;
function items(page, key) {
  const found = alt(get(page, key), []);
  if (!Array.isArray(found))
    throw new CliExit(5, `jq: error: cannot add ${typeof found} to array`);
  return found;
}
async function paginate(conn, mode, path, body, key) {
  const max = alt(body["maxResults"], 50);
  let all = [];
  let token = "";
  let start = 0;
  for (let guard = 0;guard < MAX_PAGES; guard += 1) {
    let request = { ...body, startAt: start, maxResults: max };
    if (mode === "token")
      request = token === "" ? body : { ...body, nextPageToken: token };
    const page = await lookup2(conn, path, { method: "POST", body: request });
    const got = items(page, key);
    all = [...all, ...got];
    if (mode === "token") {
      const next = alt(get(page, "nextPageToken"), "");
      token = typeof next === "string" ? next : JSON.stringify(next);
      if (get(page, "isLast") === true || token === "")
        break;
    } else {
      if (got.length === 0)
        break;
      start += got.length;
      if (start >= Number(alt(get(page, "total"), 0)))
        break;
    }
  }
  return all;
}

// plugins/jira/hooks/src/jira/search.ts
var USAGE3 = "Usage: jira search -q <JQL> [-n MAX] [--all] [--fields f1,f2]";
function issueRows(value) {
  return {
    issues: rows(value, "issues", (issue) => ({
      key: get(issue, "key"),
      summary: get(issue, "fields", "summary"),
      status: get(issue, "fields", "status", "name")
    }))
  };
}
async function search(conn, argv) {
  const flags = readFlags(argv, {
    values: { "-q": "jql", "--query": "jql", "-n": "max", "--num": "max", "--fields": "fields" },
    switches: { "--all": "all" }
  }, (arg, parsed) => {
    if ((parsed.values.get("jql") ?? "") !== "")
      unknownOption("search", arg);
    parsed.values.set("jql", arg);
  });
  const jql = flags.values.get("jql") ?? "";
  if (jql === "")
    throw new CliExit(1, USAGE3);
  const body = {
    jql,
    maxResults: numberValue(TOOL, "-n", flags.values.get("max") ?? "50")
  };
  const fields = flags.values.get("fields") ?? "";
  if (fields !== "")
    body["fields"] = fields.split(",");
  const [path, mode] = conn.version === "2" ? ["/rest/api/2/search", "offset"] : ["/rest/api/3/search/jql", "token"];
  if (flags.switches.has("all")) {
    await writeStdout(formatJson(await paginate(conn, mode, path, body, "issues")));
  } else {
    await printLean(conn, await call(conn, "POST", path, body), issueRows);
  }
  return 0;
}

// plugins/jira/hooks/src/jira/board.ts
var AGILE = "/rest/agile/1.0";
async function list2(conn, argv) {
  const flags = onlyFlags("board list", argv, {
    values: { "-p": "project", "--project": "project" }
  });
  const project = flags.values.get("project") ?? "";
  const path = `${AGILE}/board${project === "" ? "" : `?projectKeyOrId=${project}`}`;
  await printLean(conn, await call(conn, "GET", path), (value) => ({
    values: rows(value, "values", (board) => pick(board, "id", "name", "type"))
  }));
  return 0;
}
async function getBoard(conn, [id = ""]) {
  if (id === "")
    throw new CliExit(1, "Usage: jira board get <ID>");
  await printLean(conn, await call(conn, "GET", `${AGILE}/board/${id}`));
  return 0;
}
async function issues(conn, argv) {
  const [id, rest] = shiftArg(argv);
  if (id === "")
    throw new CliExit(1, "Usage: jira board issues <ID> [-q JQL] [-n MAX]");
  const flags = onlyFlags("board issues", rest, {
    values: { "-q": "jql", "--query": "jql", "-n": "max", "--num": "max" }
  });
  const params = [];
  const jql = flags.values.get("jql") ?? "";
  const max = flags.values.get("max") ?? "";
  if (jql !== "")
    params.push(["jql", jql]);
  if (max !== "")
    params.push(["maxResults", max]);
  const text = await call(conn, "GET", `${AGILE}/board/${id}/issue${encodeQuery(params)}`);
  await printLean(conn, text, issueRows);
  return 0;
}
var ACTIONS2 = { list: list2, get: getBoard, issues };
function board(conn, argv) {
  return route("Usage: jira board <list|get|issues> ...", ACTIONS2, conn, argv);
}

// plugins/jira/hooks/src/jira/adf.ts
function textBody(version, text) {
  if (version === "2")
    return text;
  return {
    type: "doc",
    version: 1,
    content: [{ type: "paragraph", content: [{ type: "text", text }] }]
  };
}

// plugins/jira/hooks/src/jira/issue.ts
var USAGE4 = "Usage: jira issue <get|create|update|delete|comment|transition|transitions|assign> ...";
var CREATE_USAGE = "Usage: jira issue create -p <PROJECT> -t <TYPE> -s <SUMMARY> [-d DESC] [--assignee ID] [-f field=val]...";
function leanIssue(issue) {
  const view = {
    key: get(issue, "key"),
    summary: get(issue, "fields", "summary"),
    status: get(issue, "fields", "status", "name"),
    assignee: get(issue, "fields", "assignee", "displayName"),
    reporter: get(issue, "fields", "reporter", "displayName"),
    priority: get(issue, "fields", "priority", "name"),
    type: get(issue, "fields", "issuetype", "name"),
    project: get(issue, "fields", "project", "key"),
    created: get(issue, "fields", "created"),
    updated: get(issue, "fields", "updated"),
    labels: get(issue, "fields", "labels")
  };
  const empty = (value) => value === null || value === "" || Array.isArray(value) && value.length === 0;
  return Object.fromEntries(Object.entries(view).filter(([, value]) => !empty(value)));
}
function assignee(conn, id) {
  return { [conn.version === "2" ? "name" : "accountId"]: id === "-" ? null : id };
}
function setFields(target, flags) {
  for (const pair of flags.lists.get("field") ?? []) {
    const at = pair.indexOf("=");
    target[at < 0 ? pair : pair.slice(0, at)] = at < 0 ? pair : pair.slice(at + 1);
  }
}
async function getIssue(conn, [key = ""]) {
  if (key === "")
    throw new CliExit(1, "Usage: jira issue get <KEY>");
  await printLean(conn, await call(conn, "GET", `${api(conn)}/issue/${key}`), leanIssue);
  return 0;
}
var EDIT_FLAGS = {
  values: { "-s": "summary", "--summary": "summary", "-d": "desc", "--desc": "desc" },
  lists: { "-f": "field", "--field": "field" }
};
async function create(conn, argv) {
  const flags = onlyFlags("issue create", argv, {
    values: {
      ...EDIT_FLAGS.values,
      "-p": "project",
      "--project": "project",
      "-t": "type",
      "--type": "type",
      "--assignee": "assignee"
    },
    lists: EDIT_FLAGS.lists
  });
  const value = (name) => flags.values.get(name) ?? "";
  if (value("project") === "" || value("type") === "" || value("summary") === "") {
    throw new CliExit(1, CREATE_USAGE);
  }
  const fields = {
    project: { key: value("project") },
    issuetype: { name: value("type") },
    summary: value("summary")
  };
  if (value("desc") !== "")
    fields["description"] = textBody(conn.version, value("desc"));
  if (value("assignee") !== "")
    fields["assignee"] = assignee(conn, value("assignee"));
  setFields(fields, flags);
  await printLean(conn, await call(conn, "POST", `${api(conn)}/issue`, { fields }));
  return 0;
}
async function update(conn, argv) {
  const [key, rest] = shiftArg(argv);
  if (key === "") {
    throw new CliExit(1, "Usage: jira issue update <KEY> [-s SUMMARY] [-d DESC] [-f field=val]...");
  }
  const flags = onlyFlags("issue update", rest, EDIT_FLAGS);
  const fields = {};
  const summary = flags.values.get("summary") ?? "";
  const desc = flags.values.get("desc") ?? "";
  if (summary !== "")
    fields["summary"] = summary;
  if (desc !== "")
    fields["description"] = textBody(conn.version, desc);
  setFields(fields, flags);
  return mutate(conn, "PUT", `${api(conn)}/issue/${key}`, { fields }, `updated ${key}`);
}
async function remove(conn, [key = ""]) {
  if (key === "")
    throw new CliExit(1, "Usage: jira issue delete <KEY>");
  return mutate(conn, "DELETE", `${api(conn)}/issue/${key}`, undefined, `deleted ${key}`);
}
async function comment(conn, [key = "", text = ""]) {
  if (key === "" || text === "")
    throw new CliExit(1, "Usage: jira issue comment <KEY> <TEXT>");
  const body = { body: textBody(conn.version, text) };
  await printLean(conn, await call(conn, "POST", `${api(conn)}/issue/${key}/comment`, body));
  return 0;
}
async function transitions(conn, [key = ""]) {
  if (key === "")
    throw new CliExit(1, "Usage: jira issue transitions <KEY>");
  const text = await call(conn, "GET", `${api(conn)}/issue/${key}/transitions`);
  await printLean(conn, text, (value) => ({
    transitions: rows(value, "transitions", (item) => pick(item, "id", "name"))
  }));
  return 0;
}
async function transitionId(conn, key, want) {
  if (/^[0-9]+$/.test(want))
    return want;
  const list = await lookup2(conn, `${api(conn)}/issue/${key}/transitions`);
  const lower = (text) => String(text).replaceAll(/[A-Z]/g, (c) => c.toLowerCase());
  const all = iterate(get(list, "transitions"));
  const ids = all.filter((item) => lower(get(item, "name")) === lower(want)).map((item) => get(item, "id"));
  if (ids.length === 0) {
    const names = all.map((item) => String(get(item, "name"))).join(", ");
    throw new CliExit(1, `jira: no transition matches '${want}'. Available: ${names}`);
  }
  if (ids.length > 1) {
    throw new CliExit(1, `jira: transition '${want}' is ambiguous (matched ids: ${ids.join(", ")})`);
  }
  return String(ids[0]);
}
async function transition(conn, [key = "", want = ""]) {
  if (key === "" || want === "")
    throw new CliExit(1, "Usage: jira issue transition <KEY> <NAME|ID>");
  const id = await transitionId(conn, key, want);
  const path = `${api(conn)}/issue/${key}/transitions`;
  return mutate(conn, "POST", path, { transition: { id } }, `transitioned ${key} -> ${id}`);
}
async function assign(conn, [key = "", who = ""]) {
  if (key === "" || who === "")
    throw new CliExit(1, "Usage: jira issue assign <KEY> <ACCOUNT_ID|->");
  const path = `${api(conn)}/issue/${key}/assignee`;
  return mutate(conn, "PUT", path, assignee(conn, who), `assigned ${key}`);
}
var ACTIONS3 = {
  get: getIssue,
  create,
  update,
  delete: remove,
  comment,
  transitions,
  transition,
  assign
};
function issue(conn, argv) {
  return route(USAGE4, ACTIONS3, conn, argv);
}

// plugins/jira/hooks/src/jira/plan.ts
import { existsSync, mkdirSync as mkdirSync2, writeFileSync as writeFileSync3 } from "fs";
import { dirname as dirname2 } from "path";

// plugins/jira/hooks/src/jira/plan-parse.ts
import { readFileSync as readFileSync2, statSync as statSync2 } from "fs";
var KEY_PATTERN = /^[A-Za-z][A-Za-z0-9_]*-[0-9]+$/;
function isFile2(path) {
  return statSync2(path, { throwIfNoEntry: false })?.isFile() === true;
}
function stepsBlock(content) {
  const lines = [];
  let inSteps = false;
  let inBlock = false;
  for (const line of content.split(`
`)) {
    if (/^## Steps \(machine-readable\)\s*$/.test(line)) {
      inSteps = true;
    } else if (inBlock && /^```\s*$/.test(line)) {
      break;
    } else if (inBlock) {
      lines.push(line);
    } else if (inSteps && /^```json\s*$/.test(line)) {
      inBlock = true;
    }
  }
  return lines.join(`
`).replace(/\n+$/, "");
}
function nonEmptyString(value) {
  return typeof value === "string" && value.length > 0;
}
function isStep(value) {
  return isObject(value) && nonEmptyString(value["id"]) && nonEmptyString(value["title"]) && nonEmptyString(value["check"]);
}
function parsed(block) {
  try {
    return JSON.parse(block);
  } catch {
    return;
  }
}
function parseSteps(doc) {
  if (!isFile2(doc))
    throw new CliExit(1, `jira plan: plan doc not found: ${doc}`);
  const block = stepsBlock(readFileSync2(doc, "utf8"));
  if (block === "") {
    throw new CliExit(1, `jira plan: no '## Steps (machine-readable)' json block in ${doc}`);
  }
  const steps = parsed(block);
  if (!Array.isArray(steps) || steps.length === 0 || !steps.every(isStep)) {
    throw new CliExit(1, `jira plan: steps block in ${doc} is not a non-empty array of {id,title,check} strings`);
  }
  return steps.map((step) => ({
    ...step,
    id: String(step["id"]),
    title: String(step["title"]),
    check: String(step["check"]),
    activity: step["activity"] === undefined || step["activity"] === false ? null : step["activity"]
  }));
}
function escaped(text) {
  return text.replaceAll(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
function docField(doc, field) {
  if (doc === "" || field === "" || !isFile2(doc))
    return "";
  const marker = `**${field}:**`;
  const line = readFileSync2(doc, "utf8").split(`
`).find((candidate) => candidate.includes(marker));
  if (line === undefined)
    return "";
  return line.replace(new RegExp(`.*\\*\\*${escaped(field)}:\\*\\*\\s*`), "").replace(/\s+\*\*[^*]+:\*\*.*$/, "").trim();
}
function issueKey(doc) {
  const key = docField(doc, "Issue");
  if (!KEY_PATTERN.test(key)) {
    throw new CliExit(1, `jira plan: doc is missing a valid '**Issue:** <KEY>' header: ${doc}`);
  }
  return key;
}

// plugins/jira/hooks/src/jira/plan-run.ts
import { spawnSync as spawnSync3 } from "child_process";
import {
  accessSync,
  closeSync,
  constants,
  mkdtempSync,
  openSync,
  readFileSync as readFileSync4,
  rmSync as rmSync2,
  statSync as statSync3
} from "fs";
import { constants as os, tmpdir as tmpdir2 } from "os";
import { join as join4 } from "path";

// plugins/jira/hooks/src/jira/plan-store.ts
import { spawnSync as spawnSync2 } from "child_process";
import { mkdirSync, readFileSync as readFileSync3, renameSync, rmSync, writeFileSync as writeFileSync2 } from "fs";
import { dirname, join as join3 } from "path";
function repoRoot(cwd) {
  const run = spawnSync2("git", ["rev-parse", "--show-toplevel"], { cwd, encoding: "utf8" });
  return run.status === 0 ? run.stdout.replace(/\n+$/, "") : cwd;
}
function isCodex(env) {
  const host = env["TOOLU_HOST_OVERRIDE"] ?? "";
  return host === "codex" || host === "" && Boolean(env["PLUGIN_ROOT"]);
}
function projectDir(env) {
  return env["TOOLU_PROJECT_CONFIG_DIRNAME"] || (isCodex(env) ? ".codex" : ".claude");
}
function requireKey(key) {
  if (!KEY_PATTERN.test(key))
    throw new CliExit(1, `jira plan: not a valid issue key: '${key}'`);
}
function ledgerPath(key, env, cwd) {
  requireKey(key);
  return join3(repoRoot(cwd), projectDir(env), "tmp", "plan-ledger", `jira-${key}.json`);
}
function docPath(key, env, cwd) {
  requireKey(key);
  return join3(repoRoot(cwd), projectDir(env), "tmp", "jira", "plans", `${key}.md`);
}
function now() {
  return new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
}
function evidence(output) {
  const tail = output.split(`
`).slice(-10).join(`
`);
  return new TextDecoder().decode(Buffer.from(tail).subarray(0, 2000));
}
function readLedger(file) {
  let content;
  try {
    content = readFileSync3(file, "utf8");
  } catch {
    return;
  }
  try {
    const value = JSON.parse(content);
    return isObject(value) ? value : undefined;
  } catch {
    return;
  }
}
function writeLedger(file, ledger) {
  const dir = dirname(file);
  try {
    mkdirSync(dir, { recursive: true });
  } catch {
    throw new CliExit(1, `jira plan: cannot create ledger dir: ${dir}`);
  }
  const temp = `${file}.tmp.${process.pid}`;
  try {
    writeFileSync2(temp, formatJson(ledger));
  } catch {
    rmSync(temp, { force: true });
    throw new CliExit(1, `jira plan: failed to stage ledger to ${temp}`);
  }
  try {
    renameSync(temp, file);
  } catch {
    rmSync(temp, { force: true });
    throw new CliExit(1, `jira plan: atomic mv failed for ${file}`);
  }
}
function count(steps, status) {
  return steps.filter((step) => step["status"] === status).length;
}
function recompute(ledger) {
  const { steps } = ledger;
  const green = count(steps, "green");
  const summary = {
    total: steps.length,
    green,
    red: count(steps, "red"),
    pending: count(steps, "pending"),
    running: count(steps, "running"),
    stale: 0,
    fresh_green: green,
    retried: 0
  };
  const next = alt(steps.find((step) => step["status"] !== "green")?.["id"], null);
  return { ...ledger, summary, next };
}
function previousSteps(prev) {
  const steps = prev?.["steps"];
  const byId = new Map;
  if (!Array.isArray(steps))
    return byId;
  for (const step of steps) {
    if (isObject(step) && typeof step["id"] === "string")
      byId.set(step["id"], step);
  }
  return byId;
}
function buildLedger(key, doc, steps, prev) {
  const old = previousSteps(prev);
  return recompute({
    version: 1,
    branch: `jira-${key}`,
    base_branch: "",
    plan_doc: doc,
    updated_at: now(),
    summary: {
      total: 0,
      green: 0,
      red: 0,
      pending: 0,
      running: 0,
      stale: 0,
      fresh_green: 0,
      retried: 0
    },
    next: null,
    steps: steps.map((step) => {
      const p = old.get(step.id) ?? {};
      return {
        id: step.id,
        title: step.title,
        check: step.check,
        status: alt(p["status"], "pending"),
        started_at: alt(p["started_at"], null),
        activity: alt(step.activity, alt(p["activity"], null)),
        exit_code: alt(p["exit_code"], null),
        diff_sha: null,
        last_run: alt(p["last_run"], null),
        evidence_tail: alt(p["evidence_tail"], null)
      };
    })
  });
}
function setStep(ledger, id, fields) {
  return recompute({
    ...ledger,
    updated_at: now(),
    steps: ledger.steps.map((step) => step["id"] === id ? { ...step, ...fields } : step)
  });
}

// plugins/jira/hooks/src/jira/plan-run.ts
function planCli(env) {
  if (env["JIRA_CLI"])
    return env["JIRA_CLI"];
  const home = env["HOME"] ?? "";
  const root = env["TOOLU_CONFIG_DIR"] || (isCodex(env) ? env["CODEX_HOME"] || join4(home, ".codex") : env["CLAUDE_CONFIG_DIR"] || join4(home, ".claude"));
  return join4(root, "jira", "jira.sh");
}
function runnable(path) {
  try {
    accessSync(path, constants.X_OK);
    return true;
  } catch {
    return statSync3(path, { throwIfNoEntry: false })?.isFile() === true;
  }
}
function statusOf(run) {
  if (run.status !== null)
    return run.status;
  const signal = run.signal === null ? undefined : os.signals[run.signal];
  return 128 + (signal ?? 0);
}
function probe(cli, env) {
  const run = spawnSync3(cli, ["user", "whoami"], { env, stdio: ["inherit", "ignore", "ignore"] });
  return run.error === undefined && run.status === 0;
}
function runCheck(cli, check, root, env) {
  const dir = mkdtempSync(join4(tmpdir2(), "jira-plan-"));
  try {
    const file = join4(dir, "out");
    const fd = openSync(file, "w");
    let run;
    try {
      run = spawnSync3("bash", ["-c", check], {
        cwd: root,
        env: { ...env, JIRA: cli },
        stdio: ["inherit", fd, fd]
      });
    } finally {
      closeSync(fd);
    }
    if (run.error !== undefined)
      return { out: `jira plan: ${run.error.message}`, code: 1 };
    return { out: readFileSync4(file, "utf8").replace(/\n+$/, ""), code: statusOf(run) };
  } finally {
    rmSync2(dir, { recursive: true, force: true });
  }
}
function runOptions(argv) {
  const flags = readFlags(argv, { values: { "--step": "step", "--activity": "activity" } }, (arg) => unknownOption("jira plan run", arg));
  return { only: flags.values.get("step") ?? "", activity: flags.values.get("activity") ?? "" };
}
async function runStep(ledger, id, state, context) {
  const check = String(ledger.steps.find((step) => step["id"] === id)?.["check"] ?? "");
  const running = { status: "running", started_at: now(), exit_code: null, evidence_tail: null };
  let current = setStep(ledger, id, context.activity === "" ? running : { ...running, activity: context.activity });
  writeLedger(state, current);
  const { out, code } = runCheck(context.cli, check, context.root, context.env);
  current = setStep(current, id, {
    status: code === 0 ? "green" : "red",
    exit_code: code,
    last_run: now(),
    evidence_tail: evidence(out),
    started_at: null
  });
  writeLedger(state, current);
  await writeStdout(code === 0 ? `green  ${id}
` : `red    ${id} (exit ${code})
`);
  return { ledger: current, green: code === 0 };
}
async function runPlan(context, argv) {
  const { only, activity } = runOptions(argv);
  const steps = parseSteps(context.doc);
  const root = repoRoot(context.cwd);
  const state = ledgerPath(context.key, context.env, context.cwd);
  let ledger = buildLedger(context.key, context.doc, steps, readLedger(state));
  if (only !== "" && !ledger.steps.some((step) => step["id"] === only)) {
    throw new CliExit(1, `jira plan run: no step '${only}' in ${context.doc}`);
  }
  const cli = planCli(context.env);
  if (!runnable(cli))
    throw new CliExit(1, `jira plan run: jira CLI not found at ${cli} (set JIRA_CLI)`);
  if (!probe(cli, context.env)) {
    throw new CliExit(1, "jira plan run: cannot reach Jira (user whoami failed) \u2014 no step statuses were written");
  }
  const targets = only === "" ? ledger.steps.map((step) => String(step["id"])) : [only];
  let failed = false;
  for (const id of targets) {
    const result = await runStep(ledger, id, state, { cli, root, env: context.env, activity });
    ledger = result.ledger;
    failed ||= !result.green;
  }
  return failed ? 1 : 0;
}

// plugins/jira/hooks/src/jira/plan.ts
function connected(context) {
  if (context.conn === undefined)
    throw new CliExit(1, "jira plan: no Jira connection");
  return context.conn;
}
function template(key, summary, doc) {
  const date = new Date().toISOString().slice(0, 10);
  return `# ${key} \u2014 ${summary}

**Date:** ${date}   **Issue:** ${key}   **Topic:** ${summary}

## Steps (machine-readable)

\`\`\`json
[]
\`\`\`

Fill the array with {"id","title","check"} objects. A \`check\` is a shell command
that exits 0 **only when Jira itself reflects the change** \u2014 assert against a live
read, e.g.

    "$JIRA" issue get ${key} --lean | jq -e '.status=="Done"' >/dev/null

\`$JIRA\` is bound to the jira CLI when the check runs. Then: jira.sh plan run ${doc}
`;
}
async function init(context, [key = ""]) {
  if (key === "")
    throw new CliExit(1, "jira plan init: needs an issue key");
  const doc = docPath(key, context.env, context.cwd);
  if (existsSync(doc))
    throw new CliExit(1, `jira plan init: ${doc} already exists`);
  const conn = connected(context);
  const issue = await lookup2(conn, `${api(conn)}/issue/${key}`);
  const summary = text(alt(get(issue, "fields", "summary"), "")) || key;
  mkdirSync2(dirname2(doc), { recursive: true });
  writeFileSync3(doc, template(key, summary, doc));
  await writeStdout(`${doc}
`);
  return 0;
}
async function status(context, [key = ""]) {
  if (key === "")
    throw new CliExit(1, "jira plan status: needs an issue key");
  const ledger = readLedger(ledgerPath(key, context.env, context.cwd));
  if (ledger === undefined) {
    throw new CliExit(1, `jira plan status: no ledger for ${key} (run: jira.sh plan run <doc>)`);
  }
  const summary = get(ledger, "summary");
  const head = `${text(get(ledger, "branch"))}  ${text(get(summary, "green"))}/${text(get(summary, "total"))}` + ` green   next: ${text(alt(get(ledger, "next"), "-"))}`;
  const steps = iterate(get(ledger, "steps")).map((step) => `  ${text(get(step, "status"))}	${text(get(step, "id"))}	${text(get(step, "title"))}`);
  await writeStdout(`${[head, ...steps].join(`
`)}
`);
  return 0;
}
async function path(context, [key = ""]) {
  if (key === "")
    throw new CliExit(1, "jira plan path: needs an issue key");
  await writeStdout(`${ledgerPath(key, context.env, context.cwd)}
`);
  return 0;
}
async function run(context, argv) {
  const [doc = "", ...rest] = argv;
  if (doc === "")
    throw new CliExit(1, "jira plan run: needs a plan doc path");
  const key = issueKey(doc);
  return runPlan({ key, doc, env: context.env, cwd: context.cwd }, rest);
}
var ACTIONS4 = { init, status, path, run };
function plan(context, argv) {
  return route(PLAN_USAGE, ACTIONS4, context, argv);
}

// plugins/jira/hooks/src/jira/project.ts
async function list3(conn) {
  await printLean(conn, await call(conn, "GET", `${api(conn)}/project`), (value) => ({
    projects: iterate(value).map((project) => pick(project, "key", "name", "id"))
  }));
  return 0;
}
function detail(action, suffix) {
  return async (conn, [key = ""]) => {
    if (key === "")
      throw new CliExit(1, `Usage: jira project ${action} <KEY>`);
    await printLean(conn, await call(conn, "GET", `${api(conn)}/project/${key}${suffix}`));
    return 0;
  };
}
var ACTIONS5 = {
  list: list3,
  get: detail("get", ""),
  versions: detail("versions", "/versions"),
  components: detail("components", "/components")
};
function project(conn, argv) {
  return route("Usage: jira project <list|get|versions|components> ...", ACTIONS5, conn, argv);
}

// plugins/jira/hooks/src/jira/raw.ts
async function raw(conn, argv) {
  const [method, path, body = ""] = argv;
  if (method === undefined || path === undefined) {
    throw new CliExit(1, "Usage: jira raw <GET|POST|PUT|DELETE> <path> [json_body]");
  }
  await printLean(conn, await call(conn, method, path, body === "" ? undefined : body));
  return 0;
}

// plugins/jira/hooks/src/jira/sprint.ts
var SPRINT = "/rest/agile/1.0/sprint";
async function list4(conn, [board = ""]) {
  if (board === "")
    throw new CliExit(1, "Usage: jira sprint list <BOARD_ID>");
  const text = await call(conn, "GET", `/rest/agile/1.0/board/${board}/sprint`);
  await printLean(conn, text, (value) => ({
    values: rows(value, "values", (sprint) => pick(sprint, "id", "name", "state"))
  }));
  return 0;
}
async function getSprint(conn, [id = ""]) {
  if (id === "")
    throw new CliExit(1, "Usage: jira sprint get <ID>");
  await printLean(conn, await call(conn, "GET", `${SPRINT}/${id}`));
  return 0;
}
async function issues2(conn, [id = ""]) {
  if (id === "")
    throw new CliExit(1, "Usage: jira sprint issues <ID>");
  await printLean(conn, await call(conn, "GET", `${SPRINT}/${id}/issue`), issueRows);
  return 0;
}
async function create2(conn, argv) {
  const [board, rest] = shiftArg(argv);
  const flags = onlyFlags("sprint create", rest, { values: { "-n": "name", "--name": "name" } });
  const name = flags.values.get("name") ?? "";
  if (board === "" || name === "") {
    throw new CliExit(1, "Usage: jira sprint create <BOARD_ID> -n <NAME>");
  }
  const body = { originBoardId: numberValue(TOOL, "BOARD_ID", board), name };
  await printLean(conn, await call(conn, "POST", SPRINT, body));
  return 0;
}
async function move(conn, argv) {
  const [id, keys] = shiftArg(argv);
  if (id === "" || keys.length === 0) {
    throw new CliExit(1, "Usage: jira sprint move <SPRINT_ID> <KEY...>");
  }
  await printLean(conn, await call(conn, "POST", `${SPRINT}/${id}/issue`, { issues: keys }));
  return 0;
}
function setState(state, verb, done) {
  return async (conn, [id = ""]) => {
    if (id === "")
      throw new CliExit(1, `Usage: jira sprint ${verb} <ID>`);
    return mutate(conn, "POST", `${SPRINT}/${id}`, { state }, `${done} sprint ${id}`);
  };
}
var ACTIONS6 = {
  list: list4,
  get: getSprint,
  issues: issues2,
  create: create2,
  move,
  start: setState("active", "start", "started"),
  complete: setState("closed", "complete", "completed")
};
function sprint(conn, argv) {
  const usage = "Usage: jira sprint <list|get|issues|create|move|start|complete> ...";
  return route(usage, ACTIONS6, conn, argv);
}

// plugins/jira/hooks/src/jira/user.ts
async function whoami(conn) {
  await printLean(conn, await call(conn, "GET", `${api(conn)}/myself`), (value) => pick(value, "accountId", "displayName", "emailAddress"));
  return 0;
}
async function search2(conn, argv) {
  const flags = onlyFlags("user search", argv, { values: { "-q": "query", "--query": "query" } });
  const query = flags.values.get("query") ?? "";
  if (query === "")
    throw new CliExit(1, "Usage: jira user search -q <QUERY>");
  const param = conn.version === "2" ? "username" : "query";
  const text = await call(conn, "GET", `${api(conn)}/user/search${encodeQuery([[param, query]])}`);
  await printLean(conn, text, (value) => ({
    users: iterate(value).map((user) => pick(user, "accountId", "displayName"))
  }));
  return 0;
}
async function getUser(conn, [id = ""]) {
  if (id === "")
    throw new CliExit(1, "Usage: jira user get <ACCOUNT_ID>");
  const param = conn.version === "2" ? "username" : "accountId";
  await printLean(conn, await call(conn, "GET", `${api(conn)}/user${encodeQuery([[param, id]])}`));
  return 0;
}
var ACTIONS7 = { whoami, search: search2, get: getUser };
function user(conn, argv) {
  return route("Usage: jira user <whoami|search|get> ...", ACTIONS7, conn, argv);
}

// plugins/jira/hooks/src/jira/worklog.ts
async function add2(conn, argv) {
  const [key, rest] = shiftArg(argv);
  const flags = onlyFlags("worklog add", rest, {
    values: { "-t": "time", "--time": "time", "-c": "comment", "--comment": "comment" }
  });
  const time = flags.values.get("time") ?? "";
  if (key === "" || time === "") {
    throw new CliExit(1, "Usage: jira worklog add <KEY> -t <TIME> [-c COMMENT]");
  }
  const body = { timeSpent: time };
  const comment = flags.values.get("comment");
  if (comment !== undefined)
    body["comment"] = textBody(conn.version, comment);
  await printLean(conn, await call(conn, "POST", `${api(conn)}/issue/${key}/worklog`, body));
  return 0;
}
async function list5(conn, [key = ""]) {
  if (key === "")
    throw new CliExit(1, "Usage: jira worklog list <KEY>");
  const text = await call(conn, "GET", `${api(conn)}/issue/${key}/worklog`);
  await printLean(conn, text, (value) => ({
    worklogs: rows(value, "worklogs", (entry) => ({
      id: get(entry, "id"),
      author: get(entry, "author", "displayName"),
      ...pick(entry, "timeSpent", "started")
    }))
  }));
  return 0;
}
async function remove2(conn, [key = "", id = ""]) {
  if (key === "" || id === "") {
    throw new CliExit(1, "Usage: jira worklog delete <KEY> <WORKLOG_ID>");
  }
  const path = `${api(conn)}/issue/${key}/worklog/${id}`;
  return mutate(conn, "DELETE", path, undefined, `deleted worklog ${id}`);
}
var ACTIONS8 = { add: add2, list: list5, delete: remove2 };
function worklog(conn, argv) {
  return route("Usage: jira worklog <add|list|delete> ...", ACTIONS8, conn, argv);
}

// plugins/jira/hooks/src/jira.ts
var FAMILIES = {
  search,
  issue,
  board,
  sprint,
  worklog,
  project,
  user,
  attachment,
  raw
};
function splitGlobals(argv) {
  const args = [];
  let version = "";
  let lean = false;
  for (let at = 0;at < argv.length; at += 1) {
    const arg = argv[at] ?? "";
    if (arg === "--api-version") {
      version = flagValue("jira", argv, at);
      at += 1;
    } else if (arg.startsWith("--api-version=")) {
      version = arg.slice("--api-version=".length);
    } else if (arg === "--lean") {
      lean = true;
    } else {
      args.push(arg);
    }
  }
  return { globals: { version, lean }, args };
}
function childEnv(conn) {
  return { ...conn.env, _JIRA_VER: conn.version, ...conn.lean ? { _JIRA_LEAN: "1" } : {} };
}
async function main() {
  const { globals, args } = splitGlobals(process.argv.slice(2));
  const [family = "", ...rest] = args;
  if (family === "")
    throw new CliExit(1, USAGE);
  const run = Object.hasOwn(FAMILIES, family) ? FAMILIES[family] : undefined;
  if (run === undefined && family !== "plan") {
    throw new CliExit(1, `jira: unknown family '${family}'
${USAGE}`);
  }
  const conn = family === "plan" && rest[0] === "path" ? undefined : connect(process.env, globals);
  if (run !== undefined && conn !== undefined)
    return run(conn, rest);
  const env = conn === undefined ? process.env : childEnv(conn);
  return plan({ conn, env, cwd: process.cwd() }, rest);
}
await runCli(main);
