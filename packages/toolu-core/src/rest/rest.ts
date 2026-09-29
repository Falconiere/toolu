/**
 * The one HTTP client of toolu's REST skill CLIs (#270). It keeps the bash
 * wrappers' contract: `curl -sS --fail-with-body` error propagation (HTTP
 * status >= 400 prints the body and exits 22), and `jq '.'` validation and
 * layout of JSON output (a non-JSON success exits 5), and no redirect
 * following (curl ran without -L, so a key header never reaches another
 * host). No retries or timeouts: the wrappers it replaces had none.
 */
import { CliExit } from "../cli/cli.ts";

export interface RestRequest {
  readonly url: string;
  readonly method?: "GET" | "POST";
  readonly headers?: Readonly<Record<string, string>>;
  /** JSON-encoded into the request body when present. */
  readonly body?: unknown;
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

/** Sends one request and returns the body text of a status below 400. */
export async function send(tool: string, request: RestRequest): Promise<string> {
  const init: RequestInit = {
    method: request.method ?? "GET",
    headers: { ...request.headers },
    redirect: "manual",
  };
  if (request.body !== undefined) init.body = JSON.stringify(request.body);
  let status: number;
  let text: string;
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
