/** API rate-limit guardrails shared by every tracker and GitHub call.
 *
 * An epic run spends one API budget across the orchestrator and every parallel
 * worker's babysit, so the orchestrator must stay cheap and back off instead
 * of burning the budget: transient failures retry with backoff, a primary
 * rate limit sleeps until reset (bounded), and launches can check the
 * remaining budget first. */

export class RateLimitError extends Error {
  constructor(
    message: string,
    readonly resetAt: number | null,
  ) {
    super(message);
    this.name = "RateLimitError";
  }
}

export type FailureClass = "rate-limit" | "transient" | "permanent";

const RATE_LIMIT =
  /rate limit|secondary rate|abuse detection|too many requests|\b429\b|RATELIMITED|quota exceeded/i;
const TRANSIENT =
  /\b50[0234]\b|timed? ?out|timeout|connection (reset|refused)|ECONNRESET|ETIMEDOUT|EAI_AGAIN|TLS handshake|unexpected EOF|temporarily unavailable/i;

export function classifyFailure(text: string): FailureClass {
  if (RATE_LIMIT.test(text)) return "rate-limit";
  if (TRANSIENT.test(text)) return "transient";
  return "permanent";
}

export type RetryPolicy = {
  attempts: number;
  baseMs: number;
  /** Longest single wait for a rate limit reset before giving up. */
  maxSleepMs: number;
  /** Resolves the epoch-ms reset time for a rate-limit failure, if known. */
  resetAt?: (err: unknown) => Promise<number | null>;
  /** False for non-idempotent writes: a 5xx may have applied, so only rate
   * limits (rejected before any effect) are retried. */
  retryTransient?: boolean;
  sleep?: (ms: number) => Promise<void>;
  now?: () => number;
};

export function envNumber(name: string, fallback: number): number {
  const v = Number(process.env[name]);
  return Number.isFinite(v) && v >= 0 ? v : fallback;
}

export function defaultPolicy(): RetryPolicy {
  return {
    attempts: envNumber("EPIC_API_ATTEMPTS", 5),
    baseMs: envNumber("EPIC_API_BACKOFF_MS", 2000),
    maxSleepMs: envNumber("EPIC_API_MAX_SLEEP_S", 900) * 1000,
  };
}

/** Run `fn`, retrying transient failures with exponential backoff and rate
 * limits until their reset (or backoff when the reset is unknown). */
export async function withRetry<T>(
  fn: () => Promise<T>,
  policy: RetryPolicy = defaultPolicy(),
): Promise<T> {
  const sleep = policy.sleep ?? ((ms: number) => Bun.sleep(ms));
  const now = policy.now ?? Date.now;
  let lastErr: unknown;
  for (let attempt = 0; attempt < policy.attempts; attempt++) {
    try {
      return await fn();
    } catch (err) {
      lastErr = err;
      const kind = classifyFailure(err instanceof Error ? err.message : String(err));
      if (kind === "permanent" || (kind === "transient" && policy.retryTransient === false)) {
        throw err;
      }
      if (attempt === policy.attempts - 1) break;
      const backoff = policy.baseMs * 2 ** attempt + Math.floor(Math.random() * policy.baseMs);
      let wait = backoff;
      if (kind === "rate-limit") {
        const reset = policy.resetAt ? await policy.resetAt(err) : null;
        if (reset !== null) {
          wait = Math.max(backoff, reset - now() + 1000);
          if (wait > policy.maxSleepMs) {
            throw new RateLimitError(
              `rate limited until ${new Date(reset).toISOString()}; ` +
                `longer than the ${Math.round(policy.maxSleepMs / 1000)}s cap`,
              reset,
            );
          }
        } else {
          // Secondary limits carry no reset; GitHub asks for at least a minute.
          wait = Math.max(backoff, 60_000);
        }
      }
      await sleep(Math.min(wait, policy.maxSleepMs));
    }
  }
  const msg = lastErr instanceof Error ? lastErr.message : String(lastErr);
  if (classifyFailure(msg) === "rate-limit") throw new RateLimitError(msg, null);
  throw lastErr instanceof Error ? lastErr : new Error(msg);
}

/** Run `fns` with at most `limit` in flight, so a large epic does not burst
 * dozens of concurrent API calls into a secondary rate limit. */
export async function mapLimit<T, R>(
  items: readonly T[],
  limit: number,
  fn: (item: T) => Promise<R>,
): Promise<R[]> {
  const out: R[] = Array.from({ length: items.length });
  let next = 0;
  const worker = async (): Promise<void> => {
    while (next < items.length) {
      const idx = next++;
      out[idx] = await fn(items[idx] as T);
    }
  };
  await Promise.all(Array.from({ length: Math.max(1, Math.min(limit, items.length)) }, worker));
  return out;
}

export type GhBudget = {
  core: { remaining: number; limit: number; reset: number };
  graphql: { remaining: number; limit: number; reset: number };
};

type RateResource = { remaining: number; limit: number; reset: number };

/** GitHub budget from `gh api rate_limit` (that endpoint does not count against
 * the budget). Returns null when gh cannot answer. Reset is epoch ms. */
export async function ghBudget(): Promise<GhBudget | null> {
  const proc = Bun.spawn(["gh", "api", "rate_limit"], { stdout: "pipe", stderr: "pipe" });
  const [out, code] = await Promise.all([new Response(proc.stdout).text(), proc.exited]);
  if (code !== 0) return null;
  try {
    const data = JSON.parse(out) as { resources?: Record<string, RateResource> };
    const core = data.resources?.core;
    const graphql = data.resources?.graphql;
    if (!core || !graphql) return null;
    const ms = (r: RateResource) => ({ ...r, reset: r.reset * 1000 });
    return { core: ms(core), graphql: ms(graphql) };
  } catch {
    return null;
  }
}

/** Earliest reset among the exhausted GitHub resources, for withRetry. */
export async function ghResetAt(): Promise<number | null> {
  const b = await ghBudget();
  if (!b) return null;
  const low = [b.core, b.graphql].filter((r) => r.remaining === 0);
  return low.length ? Math.min(...low.map((r) => r.reset)) : null;
}

/** Floors below which the orchestrator stops starting new work. Each live
 * worker's babysit polls the same token, so launches need headroom. */
export function budgetFloors(): { core: number; graphql: number } {
  return {
    core: envNumber("EPIC_GH_CORE_FLOOR", 1000),
    graphql: envNumber("EPIC_GH_GRAPHQL_FLOOR", 500),
  };
}

export function budgetLow(b: GhBudget, floors = budgetFloors()): string | null {
  if (b.core.remaining < floors.core) {
    return `GitHub core budget ${b.core.remaining}/${b.core.limit} below floor ${floors.core}`;
  }
  if (b.graphql.remaining < floors.graphql) {
    return `GitHub GraphQL budget ${b.graphql.remaining}/${b.graphql.limit} below floor ${floors.graphql}`;
  }
  return null;
}

/** fetch() JSON with retry that honors Retry-After and the Linear/Jira
 * X-RateLimit reset headers. */
export async function fetchJson(
  url: string,
  init: RequestInit,
  policy: RetryPolicy = defaultPolicy(),
): Promise<unknown> {
  let reset: number | null = null;
  return withRetry(
    async () => {
      const res = await fetch(url, init);
      const text = await res.text();
      reset = resetFromHeaders(res.headers, Date.now());
      if (!res.ok) throw new Error(`HTTP ${res.status} ${url}: ${text.slice(0, 300)}`);
      const body = text ? (JSON.parse(text) as unknown) : null;
      const errors = (body as { errors?: { extensions?: { code?: string } }[] } | null)?.errors;
      if (errors?.some((e) => e.extensions?.code === "RATELIMITED")) {
        throw new Error(`RATELIMITED ${url}`);
      }
      return body;
    },
    { ...policy, resetAt: () => Promise.resolve(reset) },
  );
}

export function resetFromHeaders(h: Headers, now: number): number | null {
  const retryAfter = h.get("retry-after");
  if (retryAfter) {
    const secs = Number(retryAfter);
    if (Number.isFinite(secs)) return now + secs * 1000;
    const at = Date.parse(retryAfter);
    if (!Number.isNaN(at)) return at;
  }
  // Linear: epoch ms. GitHub/Jira style: epoch seconds.
  const raw = h.get("x-ratelimit-requests-reset") ?? h.get("x-ratelimit-reset");
  const remaining = h.get("x-ratelimit-requests-remaining") ?? h.get("x-ratelimit-remaining");
  if (raw && remaining === "0") {
    const n = Number(raw);
    if (Number.isFinite(n)) return n > 1e12 ? n : n * 1000;
  }
  return null;
}
