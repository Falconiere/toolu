/**
 * Token counting for the deterministic tier, and run-variance stats.
 *
 * The "labeled-both" rule: exact counts from the Anthropic count_tokens API
 * when an API key is set, else a bytes/4 heuristic. If the key IS set but the
 * call fails, abort rather than silently downgrade — mixing exact and heuristic
 * counts in one delta would corrupt the comparison.
 */
import { get } from "../../json-path.ts";

export const DEFAULT_MODEL = "claude-sonnet-4-6";

type TokenCount = {
  tokens: number;
  mode: "exact" | "heuristic";
  source: "count_tokens" | "bytes-div-4";
};

export type CountOptions = {
  apiKey?: string | undefined;
  baseUrl?: string | undefined;
  model?: string;
};

export class TokenCountError extends Error {}

/** Text the way the shell harness captured it: `$(cat)` drops trailing newlines. */
export function captured(text: string): string {
  return text.replace(/\n+$/, "");
}

export async function countTokens(text: string, opts: CountOptions = {}): Promise<TokenCount> {
  const body = captured(text);
  if (opts.apiKey === undefined || opts.apiKey === "") {
    return {
      tokens: Math.trunc(Buffer.byteLength(body, "utf8") / 4),
      mode: "heuristic",
      source: "bytes-div-4",
    };
  }
  const base =
    opts.baseUrl === undefined || opts.baseUrl === "" ? "https://api.anthropic.com" : opts.baseUrl;
  let response: Response;
  try {
    response = await fetch(`${base}/v1/messages/count_tokens`, {
      method: "POST",
      headers: {
        "x-api-key": opts.apiKey,
        "anthropic-version": "2023-06-01",
        "content-type": "application/json",
      },
      body: JSON.stringify({
        model: opts.model ?? DEFAULT_MODEL,
        messages: [{ role: "user", content: body }],
      }),
      signal: AbortSignal.timeout(30_000),
    });
  } catch {
    throw new TokenCountError(
      "count_tokens request failed (key set; refusing to downgrade to heuristic)",
    );
  }
  const raw = await response.text();
  let tokens: unknown;
  try {
    tokens = get(JSON.parse(raw), "input_tokens");
  } catch {
    tokens = undefined;
  }
  if (typeof tokens !== "number")
    throw new TokenCountError(`no input_tokens in API response: ${raw}`);
  return { tokens: Math.trunc(tokens), mode: "exact", source: "count_tokens" };
}

type Stats = { mean: number; stddev: number; n: number };

/** Sample standard deviation (n-1); one sample has stddev 0; none reports zeros. */
export function stats(samples: readonly number[]): Stats {
  const n = samples.length;
  if (n === 0) return { mean: 0, stddev: 0, n: 0 };
  const [first = 0] = samples;
  if (n === 1) return { mean: first, stddev: 0, n: 1 };
  const mean = samples.reduce((a, b) => a + b, 0) / n;
  const variance = samples.map((x) => (x - mean) * (x - mean)).reduce((a, b) => a + b, 0) / (n - 1);
  return { mean, stddev: Math.sqrt(variance), n };
}
