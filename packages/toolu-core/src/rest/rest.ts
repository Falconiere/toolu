/**
 * The one HTTP client of toolu's REST skill CLIs (#270). It keeps the bash
 * wrappers' contract: `curl -sS --fail-with-body` error propagation (HTTP
 * status >= 400 prints the body and exits 22), and `jq '.'` validation and
 * layout of JSON output (a non-JSON success exits 5), and no redirect
 * following (curl ran without -L, so a key header never reaches another
 * host). `download` is the one `curl -L` path (#272): it follows redirects but
 * drops Authorization once the origin changes, as curl does. No retries or
 * timeouts: the wrappers it replaces had none.
 */
import { CliExit } from "../cli/cli.ts";

export interface RestRequest {
  readonly url: string;
  /** Any HTTP method, as `curl -X` took one; GET by default. */
  readonly method?: string;
  readonly headers?: Readonly<Record<string, string>>;
  /** JSON-encoded into the request body when present. */
  readonly body?: unknown;
  /** Sent as-is in place of `body`: a caller's raw JSON text (`curl --data`) or a multipart form (`curl -F`). */
  readonly payload?: string | FormData;
  /** False when the caller prints responses raw: an error body is then passed through unformatted. */
  readonly json?: boolean;
}

/** `value` laid out like `jq '.'`: two-space indent and a trailing newline. */
export function formatJson(value: unknown): string {
  return `${JSON.stringify(value, null, 2)}\n`;
}

function tryParse(text: string): { ok: true; value: unknown } | { ok: false } {
  try {
    const value: unknown = JSON.parse(text);
    return { ok: true, value };
  } catch {
    return { ok: false };
  }
}

/** The parsed body, or exit 5 when a success body is not JSON. */
export function parseJson(tool: string, text: string): unknown {
  const parsed = tryParse(text);
  if (!parsed.ok) throw new CliExit(5, `${tool}: response is not JSON`);
  return parsed.value;
}

/**
 * What `jq '<projection>'` printed for a success body: nothing for an empty
 * body (a 204, say), else the projected value laid out by `formatJson`.
 */
export function jsonOutput(
  tool: string,
  text: string,
  project: (value: unknown) => unknown = (value) => value,
): string {
  if (text.trim() === "") return "";
  return formatJson(project(parseJson(tool, text)));
}

function reason(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** One fetch with redirects left to the caller; a transport failure exits 1. */
async function exchange(tool: string, url: string, init: RequestInit): Promise<Response> {
  try {
    return await fetch(url, { ...init, redirect: "manual" });
  } catch (error) {
    throw new CliExit(1, `${tool}: request failed: ${reason(error)}`);
  }
}

/** Reads a response body; a connection dropped mid-body is a transport failure too. */
async function readBody<T>(tool: string, read: () => Promise<T>): Promise<T> {
  try {
    return await read();
  } catch (error) {
    throw new CliExit(1, `${tool}: request failed: ${reason(error)}`);
  }
}

/** Sends one request and returns the body text of a status below 400. */
export async function send(tool: string, request: RestRequest): Promise<string> {
  const init: RequestInit = { method: request.method ?? "GET", headers: { ...request.headers } };
  if (request.payload !== undefined) init.body = request.payload;
  else if (request.body !== undefined) init.body = JSON.stringify(request.body);
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

const REDIRECTS = new Set([301, 302, 303, 307, 308]);
/** curl's default --max-redirs. */
const MAX_REDIRECTS = 50;

function withoutAuthorization(headers: Record<string, string>): Record<string, string> {
  return Object.fromEntries(
    Object.entries(headers).filter(([name]) => name.toLowerCase() !== "authorization"),
  );
}

interface Hop {
  readonly url: string;
  readonly headers: Record<string, string>;
  readonly origin: string;
  /** Redirects followed so far. */
  readonly count: number;
}

/** A hop's URL for messages: no query, which may carry a signed media token. */
function shown(url: string): string {
  const parsed = new URL(url);
  return `${parsed.origin}${parsed.pathname}`;
}

/** One GET; a redirect recurses into the next hop, which is sequential by nature. */
async function follow(tool: string, hop: Hop): Promise<Uint8Array> {
  const response = await exchange(tool, hop.url, { method: "GET", headers: hop.headers });
  const location = response.headers.get("location");
  if (REDIRECTS.has(response.status) && location !== null) {
    await response.body?.cancel();
    if (hop.count >= MAX_REDIRECTS)
      throw new CliExit(47, `${tool}: too many redirects from ${shown(hop.url)}`);
    const url = new URL(location, hop.url).href;
    const headers =
      new URL(url).origin === hop.origin ? hop.headers : withoutAuthorization(hop.headers);
    return follow(tool, { ...hop, url, headers, count: hop.count + 1 });
  }
  const bytes = new Uint8Array(await readBody(tool, () => response.arrayBuffer()));
  if (response.status >= 400) {
    const body = new TextDecoder().decode(bytes);
    throw new CliExit(22, `${tool}: HTTP ${response.status} from ${shown(hop.url)}`, body);
  }
  return bytes;
}

/**
 * GETs `request.url` like `curl -sS --fail-with-body -L` and returns the body
 * bytes. Redirects are followed (Jira serves attachment content from a media
 * host), and Authorization is dropped once a hop leaves the original origin.
 * Too many redirects exits 47, curl's status for it. Error messages name the
 * failing URL without its query string.
 */
export function download(tool: string, request: RestRequest): Promise<Uint8Array> {
  const origin = new URL(request.url).origin;
  return follow(tool, { url: request.url, headers: { ...request.headers }, origin, count: 0 });
}

/** Percent-encodes like python's `quote(value, safe="")`: only RFC 3986 unreserved characters stay. */
function quote(value: string): string {
  return encodeURIComponent(value).replaceAll(
    /[!'()*]/g,
    (char) => `%${char.charCodeAt(0).toString(16).toUpperCase()}`,
  );
}

/** `?k=v&k2=v2` with each value quoted; keys are literal. Empty when there are no params. */
export function encodeQuery(params: ReadonlyArray<readonly [string, string]>): string {
  if (params.length === 0) return "";
  return `?${params.map(([key, value]) => `${key}=${quote(value)}`).join("&")}`;
}
