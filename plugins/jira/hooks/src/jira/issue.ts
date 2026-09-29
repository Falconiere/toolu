/**
 * Issue operations: get, create, update, delete, comment, transition(s),
 * assign. Comment/description bodies are version-gated (ADF for v3, plain
 * text for v2); v3 keys people by accountId, v2 by name.
 */
import { CliExit } from "@toolu/core/cli";
import { textBody } from "./adf.ts";
import { type Action, type Flags, onlyFlags, route, shiftArg } from "./flags.ts";
import { type Conn, api, call, lookup, mutate, printLean } from "./http.ts";
import { get, iterate, type JsonObject, pick, rows } from "./jq.ts";

const USAGE =
  "Usage: jira issue <get|create|update|delete|comment|transition|transitions|assign> ...";
const CREATE_USAGE =
  "Usage: jira issue create -p <PROJECT> -t <TYPE> -s <SUMMARY> [-d DESC] [--assignee ID] [-f field=val]...";

/** The lean view of one issue: useful fields, with null/[]/"" values dropped. */
function leanIssue(issue: unknown): unknown {
  const view: JsonObject = {
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
    labels: get(issue, "fields", "labels"),
  };
  const empty = (value: unknown) =>
    value === null || value === "" || (Array.isArray(value) && value.length === 0);
  return Object.fromEntries(Object.entries(view).filter(([, value]) => !empty(value)));
}

/** An assignee value; "-" unassigns with null. */
function assignee(conn: Conn, id: string): JsonObject {
  return { [conn.version === "2" ? "name" : "accountId"]: id === "-" ? null : id };
}

/** `-f k=v` pairs set as string fields; with no `=`, bash used the whole text for both. */
function setFields(target: JsonObject, flags: Flags): void {
  for (const pair of flags.lists.get("field") ?? []) {
    const at = pair.indexOf("=");
    target[at < 0 ? pair : pair.slice(0, at)] = at < 0 ? pair : pair.slice(at + 1);
  }
}

async function getIssue(conn: Conn, [key = ""]: readonly string[]): Promise<number> {
  if (key === "") throw new CliExit(1, "Usage: jira issue get <KEY>");
  await printLean(conn, await call(conn, "GET", `${api(conn)}/issue/${key}`), leanIssue);
  return 0;
}

const EDIT_FLAGS = {
  values: { "-s": "summary", "--summary": "summary", "-d": "desc", "--desc": "desc" },
  lists: { "-f": "field", "--field": "field" },
};

async function create(conn: Conn, argv: readonly string[]): Promise<number> {
  const flags = onlyFlags("issue create", argv, {
    values: {
      ...EDIT_FLAGS.values,
      "-p": "project",
      "--project": "project",
      "-t": "type",
      "--type": "type",
      "--assignee": "assignee",
    },
    lists: EDIT_FLAGS.lists,
  });
  const value = (name: string) => flags.values.get(name) ?? "";
  if (value("project") === "" || value("type") === "" || value("summary") === "") {
    throw new CliExit(1, CREATE_USAGE);
  }
  const fields: JsonObject = {
    project: { key: value("project") },
    issuetype: { name: value("type") },
    summary: value("summary"),
  };
  if (value("desc") !== "") fields["description"] = textBody(conn.version, value("desc"));
  if (value("assignee") !== "") fields["assignee"] = assignee(conn, value("assignee"));
  setFields(fields, flags);
  await printLean(conn, await call(conn, "POST", `${api(conn)}/issue`, { fields }));
  return 0;
}

async function update(conn: Conn, argv: readonly string[]): Promise<number> {
  const [key, rest] = shiftArg(argv);
  if (key === "") {
    throw new CliExit(1, "Usage: jira issue update <KEY> [-s SUMMARY] [-d DESC] [-f field=val]...");
  }
  const flags = onlyFlags("issue update", rest, EDIT_FLAGS);
  const fields: JsonObject = {};
  const summary = flags.values.get("summary") ?? "";
  const desc = flags.values.get("desc") ?? "";
  if (summary !== "") fields["summary"] = summary;
  if (desc !== "") fields["description"] = textBody(conn.version, desc);
  setFields(fields, flags);
  return mutate(conn, "PUT", `${api(conn)}/issue/${key}`, { fields }, `updated ${key}`);
}

async function remove(conn: Conn, [key = ""]: readonly string[]): Promise<number> {
  if (key === "") throw new CliExit(1, "Usage: jira issue delete <KEY>");
  return mutate(conn, "DELETE", `${api(conn)}/issue/${key}`, undefined, `deleted ${key}`);
}

async function comment(conn: Conn, [key = "", text = ""]: readonly string[]): Promise<number> {
  if (key === "" || text === "") throw new CliExit(1, "Usage: jira issue comment <KEY> <TEXT>");
  const body = { body: textBody(conn.version, text) };
  await printLean(conn, await call(conn, "POST", `${api(conn)}/issue/${key}/comment`, body));
  return 0;
}

async function transitions(conn: Conn, [key = ""]: readonly string[]): Promise<number> {
  if (key === "") throw new CliExit(1, "Usage: jira issue transitions <KEY>");
  const text = await call(conn, "GET", `${api(conn)}/issue/${key}/transitions`);
  await printLean(conn, text, (value) => ({
    transitions: rows(value, "transitions", (item) => pick(item, "id", "name")),
  }));
  return 0;
}

/** A transition id: digits as given, else the one transition whose name matches (ASCII case-insensitive). */
async function transitionId(conn: Conn, key: string, want: string): Promise<string> {
  if (/^[0-9]+$/.test(want)) return want;
  const list = await lookup(conn, `${api(conn)}/issue/${key}/transitions`);
  const lower = (text: unknown) => String(text).replaceAll(/[A-Z]/g, (c) => c.toLowerCase());
  const all = iterate(get(list, "transitions"));
  const ids = all
    .filter((item) => lower(get(item, "name")) === lower(want))
    .map((item) => get(item, "id"));
  if (ids.length === 0) {
    const names = all.map((item) => String(get(item, "name"))).join(", ");
    throw new CliExit(1, `jira: no transition matches '${want}'. Available: ${names}`);
  }
  if (ids.length > 1) {
    throw new CliExit(
      1,
      `jira: transition '${want}' is ambiguous (matched ids: ${ids.join(", ")})`,
    );
  }
  return String(ids[0]);
}

async function transition(conn: Conn, [key = "", want = ""]: readonly string[]): Promise<number> {
  if (key === "" || want === "")
    throw new CliExit(1, "Usage: jira issue transition <KEY> <NAME|ID>");
  const id = await transitionId(conn, key, want);
  const path = `${api(conn)}/issue/${key}/transitions`;
  return mutate(conn, "POST", path, { transition: { id } }, `transitioned ${key} -> ${id}`);
}

async function assign(conn: Conn, [key = "", who = ""]: readonly string[]): Promise<number> {
  if (key === "" || who === "")
    throw new CliExit(1, "Usage: jira issue assign <KEY> <ACCOUNT_ID|->");
  const path = `${api(conn)}/issue/${key}/assignee`;
  return mutate(conn, "PUT", path, assignee(conn, who), `assigned ${key}`);
}

const ACTIONS: Readonly<Record<string, Action<Conn>>> = {
  get: getIssue,
  create,
  update,
  delete: remove,
  comment,
  transitions,
  transition,
  assign,
};

export function issue(conn: Conn, argv: readonly string[]): Promise<number> {
  return route(USAGE, ACTIONS, conn, argv);
}
