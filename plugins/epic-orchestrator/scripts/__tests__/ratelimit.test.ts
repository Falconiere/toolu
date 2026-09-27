/** Retry, backoff, and rate-limit reset handling. Timings are real but tiny. */

import { describe, expect, test } from "bun:test";
import {
  RateLimitError,
  budgetLow,
  classifyFailure,
  fetchJson,
  mapLimit,
  resetFromHeaders,
  withRetry,
} from "../ratelimit.ts";

const FAST = { attempts: 4, baseMs: 1, maxSleepMs: 2000 };

describe("classifyFailure", () => {
  test("gh, curl, and Linear rate-limit texts", () => {
    expect(classifyFailure("gh: API rate limit exceeded for user ID 1. (HTTP 403)")).toBe(
      "rate-limit",
    );
    expect(classifyFailure("You have exceeded a secondary rate limit")).toBe("rate-limit");
    expect(classifyFailure("curl: (22) The requested URL returned error: 429")).toBe("rate-limit");
    expect(classifyFailure("RATELIMITED https://api.linear.app/graphql")).toBe("rate-limit");
  });

  test("5xx and network errors are transient; 404 is permanent", () => {
    expect(classifyFailure("HTTP 502: Bad Gateway")).toBe("transient");
    expect(classifyFailure("read tcp: connection reset by peer")).toBe("transient");
    expect(classifyFailure("HTTP 404: Not Found")).toBe("permanent");
  });
});

describe("withRetry", () => {
  test("retries transient failures then succeeds", async () => {
    let calls = 0;
    const out = await withRetry(() => {
      calls++;
      return calls < 3 ? Promise.reject(new Error("HTTP 503")) : Promise.resolve("ok");
    }, FAST);
    expect(out).toBe("ok");
    expect(calls).toBe(3);
  });

  test("permanent failure is not retried", async () => {
    let calls = 0;
    const run = withRetry(() => {
      calls++;
      return Promise.reject(new Error("HTTP 404"));
    }, FAST);
    expect(run).rejects.toThrow("HTTP 404");
    await run.catch(() => undefined);
    expect(calls).toBe(1);
  });

  test("writes do not retry a 5xx that may have applied", async () => {
    let calls = 0;
    const run = withRetry(
      () => {
        calls++;
        return Promise.reject(new Error("HTTP 502"));
      },
      { ...FAST, retryTransient: false },
    );
    await run.catch(() => undefined);
    expect(calls).toBe(1);
  });

  test("rate limit waits for the reset it is given", async () => {
    let calls = 0;
    const started = Date.now();
    const out = await withRetry(
      () => {
        calls++;
        return calls === 1 ? Promise.reject(new Error("rate limit")) : Promise.resolve(calls);
      },
      { ...FAST, resetAt: () => Promise.resolve(Date.now() - 900) },
    );
    expect(out).toBe(2);
    expect(Date.now() - started).toBeGreaterThanOrEqual(90);
  });

  test("a reset beyond the cap raises RateLimitError instead of sleeping", async () => {
    const far = Date.now() + 3_600_000;
    const run = withRetry(() => Promise.reject(new Error("API rate limit exceeded")), {
      ...FAST,
      resetAt: () => Promise.resolve(far),
    });
    const err = await run.catch((e: unknown) => e);
    expect(err).toBeInstanceOf(RateLimitError);
    expect((err as RateLimitError).resetAt).toBe(far);
  });
});

describe("resetFromHeaders", () => {
  const now = 1_700_000_000_000;
  test("Retry-After HTTP-date (always GMT)", () => {
    const h = new Headers({ "retry-after": "Wed, 21 Oct 2015 07:28:00 GMT" });
    expect(resetFromHeaders(h, now)).toBe(Date.UTC(2015, 9, 21, 7, 28, 0));
  });
  test("Retry-After seconds", () => {
    expect(resetFromHeaders(new Headers({ "retry-after": "30" }), now)).toBe(now + 30_000);
  });
  test("Linear epoch-ms reset only when exhausted", () => {
    const h = new Headers({
      "x-ratelimit-requests-remaining": "0",
      "x-ratelimit-requests-reset": String(now + 5000),
    });
    expect(resetFromHeaders(h, now)).toBe(now + 5000);
    h.set("x-ratelimit-requests-remaining", "10");
    expect(resetFromHeaders(h, now)).toBeNull();
  });
  test("GitHub epoch-seconds reset", () => {
    const h = new Headers({ "x-ratelimit-remaining": "0", "x-ratelimit-reset": "1700000100" });
    expect(resetFromHeaders(h, now)).toBe(1_700_000_100_000);
  });
});

test("mapLimit keeps order and caps concurrency", async () => {
  let live = 0;
  let peak = 0;
  const out = await mapLimit([5, 1, 4, 2, 3], 2, async (n) => {
    live++;
    peak = Math.max(peak, live);
    await Bun.sleep(n);
    live--;
    return n * 10;
  });
  expect(out).toEqual([50, 10, 40, 20, 30]);
  expect(peak).toBe(2);
});

test("budgetLow names the exhausted resource", () => {
  const b = {
    core: { remaining: 400, limit: 5000, reset: 0 },
    graphql: { remaining: 4000, limit: 5000, reset: 0 },
  };
  expect(budgetLow(b, { core: 1000, graphql: 500 })).toContain("core budget 400/5000");
  expect(budgetLow(b, { core: 100, graphql: 500 })).toBeNull();
});

test("fetchJson waits out a real 429 with Retry-After, then succeeds", async () => {
  let hits = 0;
  const server = Bun.serve({
    port: 0,
    fetch() {
      hits++;
      if (hits === 1) {
        return new Response("slow down", { status: 429, headers: { "retry-after": "0" } });
      }
      if (hits === 2) {
        return Response.json({ errors: [{ message: "x", extensions: { code: "RATELIMITED" } }] });
      }
      return Response.json({ data: { ok: true } });
    },
  });
  try {
    const out = await fetchJson(
      `http://localhost:${server.port}/graphql`,
      { method: "POST" },
      FAST,
    );
    expect(out).toEqual({ data: { ok: true } });
    expect(hits).toBe(3);
  } finally {
    await server.stop(true);
  }
});

test("fetchJson does not retry a 404", async () => {
  let hits = 0;
  const server = Bun.serve({
    port: 0,
    fetch() {
      hits++;
      return new Response("nope", { status: 404 });
    },
  });
  try {
    const err = await fetchJson(`http://localhost:${server.port}/x`, {}, FAST).catch(
      (e: unknown) => e,
    );
    expect(String(err)).toContain("HTTP 404");
    expect(hits).toBe(1);
  } finally {
    await server.stop(true);
  }
});

test("mapLimit rejects on the first failure, including a synchronous throw", async () => {
  const seen: number[] = [];
  const run = mapLimit([1, 2, 3, 4], 2, (n) => {
    seen.push(n);
    if (n === 2) throw new Error("bad item 2");
    return Promise.resolve(n);
  });
  expect(run).rejects.toThrow("bad item 2");
  await run.catch(() => undefined);
  expect(seen).toContain(2);
});
