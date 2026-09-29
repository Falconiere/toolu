/**
 * Issue attachments: add (multipart upload), list, get (metadata), download
 * (binary content, following Jira's redirect to its media host), read
 * (download, then print text-like content as context or report binaries).
 */
import { CliExit, writeStdout } from "@toolu/core/cli";
import { download, send } from "@toolu/core/rest";
import { randomBytes } from "node:crypto";
import { statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { type Action, onlyFlags, route, shiftArg } from "./flags.ts";
import { type Conn, TOOL, api, call, jiraRequest, lookup, printLean } from "./http.ts";
import { alt, get, pick, rows, text } from "./jq.ts";

const USAGE = "Usage: jira attachment <add|list|get|download|read> ...";

function isFile(path: string): boolean {
  return statSync(path, { throwIfNoEntry: false })?.isFile() === true;
}

async function add(conn: Conn, [key = "", file = ""]: readonly string[]): Promise<number> {
  if (key === "" || file === "") throw new CliExit(1, "Usage: jira attachment add <KEY> <FILE>");
  if (!isFile(file)) throw new CliExit(1, `attachment add: file not found: ${file}`);
  const form = new FormData();
  form.append("file", Bun.file(file), basename(file));
  const request = jiraRequest(conn, "POST", `${api(conn)}/issue/${key}/attachments`);
  const headers = { ...request.headers, "X-Atlassian-Token": "no-check" };
  await printLean(conn, await send(TOOL, { ...request, headers, payload: form }));
  return 0;
}

async function list(conn: Conn, [key = ""]: readonly string[]): Promise<number> {
  if (key === "") throw new CliExit(1, "Usage: jira attachment list <KEY>");
  const body = await call(conn, "GET", `${api(conn)}/issue/${key}?fields=attachment`);
  await printLean(conn, body, (value) => ({
    attachments: rows(get(value, "fields"), "attachment", (item) =>
      pick(item, "id", "filename", "size"),
    ),
  }));
  return 0;
}

async function getAttachment(conn: Conn, [id = ""]: readonly string[]): Promise<number> {
  if (id === "") throw new CliExit(1, "Usage: jira attachment get <ATTACHMENT_ID>");
  await printLean(conn, await call(conn, "GET", `${api(conn)}/attachment/${id}`));
  return 0;
}

/** The attachment's bytes, with auth, following redirects (bash `jira_download`). */
function content(conn: Conn, id: string): Promise<Uint8Array> {
  const request = jiraRequest(conn, "GET", `${api(conn)}/attachment/content/${id}`);
  return download(TOOL, { ...request, headers: { ...request.headers, Accept: "*/*" } });
}

async function save(conn: Conn, argv: readonly string[]): Promise<number> {
  const [id, rest] = shiftArg(argv);
  const flags = onlyFlags("attachment download", rest, {
    values: { "-o": "out", "--output": "out" },
  });
  if (id === "") {
    throw new CliExit(1, "Usage: jira attachment download <ATTACHMENT_ID> [-o OUTFILE]");
  }
  let out = flags.values.get("out") ?? "";
  if (out === "") {
    // bash's `fn=$(… | jq -r '.filename // empty')` failed the script with curl's status.
    const meta = await lookup(conn, `${api(conn)}/attachment/${id}`, { code: 22 });
    // Only the name part: a metadata filename such as ../../.bashrc must not
    // steer the write outside the working directory.
    const name = basename(text(alt(get(meta, "filename"), "")));
    out = name === "" || name === "." || name === ".." ? `attachment-${id}` : name;
  }
  writeFileSync(out, await content(conn, id));
  await writeStdout(`downloaded attachment ${id} -> ${out}\n`);
  return 0;
}

const TEXT_TYPES = new Set([
  "application/json",
  "application/xml",
  "application/javascript",
  "application/x-yaml",
  "application/yaml",
]);

function isTextLike(mime: string): boolean {
  return (
    mime.startsWith("text/") ||
    TEXT_TYPES.has(mime) ||
    mime.endsWith("+json") ||
    mime.endsWith("+xml")
  );
}

/** A fresh owner-only file in the temp dir, like `mktemp -t jira-attachment.XXXXXX`. */
function tempFile(bytes: Uint8Array): string {
  const path = join(tmpdir(), `jira-attachment.${randomBytes(6).toString("hex")}`);
  writeFileSync(path, bytes, { flag: "wx", mode: 0o600 });
  return path;
}

async function read(conn: Conn, [id = ""]: readonly string[]): Promise<number> {
  if (id === "") throw new CliExit(1, "Usage: jira attachment read <ATTACHMENT_ID>");
  const meta = await lookup(conn, `${api(conn)}/attachment/${id}`);
  const mime = text(alt(get(meta, "mimeType"), ""));
  const name = text(alt(get(meta, "filename"), "attachment"));
  let bytes: Uint8Array;
  try {
    bytes = await content(conn, id);
  } catch (error) {
    if (error instanceof CliExit && error.code === 22) throw new CliExit(1, error.message);
    throw error;
  }
  if (isTextLike(mime)) {
    await writeStdout(`# ${name} (${mime})\n\n`);
    await writeStdout(bytes);
    return 0;
  }
  // The saved copy is kept for the caller, as bash kept its mktemp file.
  const saved = tempFile(bytes);
  await writeStdout(
    `${name} — binary attachment (${mime || "unknown"}, ${bytes.length} bytes) saved to ${saved}\n` +
      `Run \`jira attachment download ${id} -o <path>\` to save it elsewhere.\n`,
  );
  return 0;
}

const ACTIONS: Readonly<Record<string, Action<Conn>>> = {
  add,
  list,
  get: getAttachment,
  download: save,
  read,
};

export function attachment(conn: Conn, argv: readonly string[]): Promise<number> {
  return route(USAGE, ACTIONS, conn, argv);
}
