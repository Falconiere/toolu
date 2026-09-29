/** LinearTracker end to end against a local stand-in for api.linear.app:
 * the auth header each key type sends, the epic/children queries, and the
 * close flow (comment, then move to the team's first completed state). */

import { expect, test } from "bun:test";
import { LinearTracker, linearAuthHeader } from "../trackers/linear.ts";

type Req = { auth: string | null; query: string; variables: Record<string, unknown> };

const CHILD = {
  id: "uuid-5",
  identifier: "ENG-5",
  title: "Add refunds API",
  url: "https://linear.app/acme/issue/ENG-5",
  description: "Repo: acme/payments\nImplement refunds.",
  state: { type: "started" },
  labels: { nodes: [{ name: "backend" }] },
  inverseRelations: {
    nodes: [{ type: "blocks", issue: { identifier: "ENG-4", state: { type: "completed" } } }],
  },
};

function answer(query: string): unknown {
  if (query.includes("children(")) {
    return {
      issue: { children: { nodes: [CHILD], pageInfo: { hasNextPage: false, endCursor: null } } },
    };
  }
  if (query.includes("team{states")) {
    return {
      issue: {
        id: "uuid-5",
        state: { type: "started" },
        team: {
          states: {
            nodes: [
              { id: "st-late", position: 2 },
              { id: "st-done", position: 1 },
            ],
          },
        },
      },
    };
  }
  if (query.includes("commentCreate")) return { commentCreate: { success: true } };
  if (query.includes("issueUpdate")) return { issueUpdate: { success: true } };
  return {
    issue: {
      id: "uuid-1",
      identifier: "ENG-1",
      title: "Refunds epic",
      url: "https://linear.app/acme/issue/ENG-1",
      state: { type: "started" },
    },
  };
}

type Stub = { requests: Req[]; url: string; stop: () => Promise<void> };

/** A local stand-in for api.linear.app; each test starts and stops its own. */
function startStub(): Stub {
  const requests: Req[] = [];
  const server = Bun.serve({
    port: 0,
    async fetch(req) {
      const body = (await req.json()) as { query: string; variables: Record<string, unknown> };
      requests.push({ auth: req.headers.get("authorization"), ...body });
      return Response.json({ data: answer(body.query) });
    },
  });
  return {
    requests,
    url: `http://localhost:${server.port}/graphql`,
    stop: () => server.stop(true),
  };
}

function tracker(key: string, url: string): LinearTracker {
  return new LinearTracker("linear:ENG-1", "acme/mono", {
    LINEAR_API_KEY: key,
    LINEAR_API_URL: url,
  });
}

test.concurrent("auth header: personal API key is sent bare, OAuth token as Bearer", async () => {
  const stub = startStub();
  try {
    await tracker("lin_api_personal123", stub.url).epic();
    await tracker("lin_oauth_token456", stub.url).epic();
    expect(stub.requests.map((r) => r.auth)).toEqual([
      "lin_api_personal123",
      "Bearer lin_oauth_token456",
    ]);
  } finally {
    await stub.stop();
  }
});

test.concurrent("auth header: an explicit Bearer value is passed through unchanged", () => {
  expect(linearAuthHeader("Bearer abc")).toBe("Bearer abc");
  expect(linearAuthHeader("  lin_api_x  ")).toBe("lin_api_x");
});

test.concurrent("auth header: a missing key is refused before any request", () => {
  expect(
    () => new LinearTracker("ENG-1", "acme/mono", { LINEAR_API_URL: "http://localhost:1/graphql" }),
  ).toThrow("LINEAR_API_KEY is not set");
});

test.concurrent("epic and children are normalized from the GraphQL payload", async () => {
  const stub = startStub();
  try {
    const t = tracker("lin_api_k", stub.url);
    expect(await t.epic()).toEqual({
      ref: "ENG-1",
      title: "Refunds epic",
      state: "open",
      url: "https://linear.app/acme/issue/ENG-1",
    });
    const [child] = await t.children();
    expect(child).toEqual({
      ref: "ENG-5",
      title: "Add refunds API",
      state: "open",
      url: "https://linear.app/acme/issue/ENG-5",
      repo: "acme/payments",
      number: null,
      blockers: { "ENG-4": "closed" },
      prs: [],
      deps_source: "relations",
      labels: ["backend"],
      excerpt: "Repo: acme/payments\nImplement refunds.",
    });
    expect(stub.requests[1]?.variables).toEqual({ id: "ENG-1", after: null });
  } finally {
    await stub.stop();
  }
});

test.concurrent("closing comments first, then moves to the lowest-position completed state", async () => {
  const stub = startStub();
  try {
    expect(await tracker("lin_api_k", stub.url).closeIssue("ENG-5", "Delivered in PR 7")).toBe(
      "completed",
    );
    const steps = stub.requests.map((r) =>
      r.query.includes("commentCreate")
        ? "comment"
        : r.query.includes("issueUpdate")
          ? "update"
          : "read",
    );
    expect(steps).toEqual(["read", "comment", "update"]);
    expect(stub.requests[1]?.variables).toEqual({ i: "uuid-5", b: "Delivered in PR 7" });
    expect(stub.requests[2]?.variables).toEqual({ id: "uuid-5", s: "st-done" });
  } finally {
    await stub.stop();
  }
});
