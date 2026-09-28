/**
 * The committed context7 bundle, run by path as the published symlink runs it,
 * against the loopback HTTPS fixture posing as context7.com.
 */
import { afterAll, beforeEach, expect, test } from "bun:test";
import { startHttpsFixture } from "@toolu/conformance/https-fixture";
import { join } from "node:path";
import { DOCS_USAGE, MAIN_USAGE, SEARCH_USAGE } from "../context7/usage.ts";

const BUNDLE = join(import.meta.dir, "../../dist/search.js");
const fixture = await startHttpsFixture(["context7.com"]);

afterAll(() => fixture.stop());
beforeEach(() => fixture.plan([]));

// The ctx7sk_ prefix is load-bearing, but a literal ctx7sk_<random> reads as a
// live credential to secret scanners: assemble it from an inert suffix.
const PREFIX = "ctx7sk_";
const KEY = `${PREFIX}not-a-real-key`;

async function context7(args: readonly string[], key: string | null = null) {
  const env: Record<string, string | undefined> = { ...process.env, ...fixture.env };
  // Never inherit a developer's real key.
  delete env["CONTEXT7_API_KEY"];
  if (key !== null) env["CONTEXT7_API_KEY"] = key;
  const child = Bun.spawn([BUNDLE, ...args], { env, stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, status] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  return { status, stdout, stderr };
}

function onlyRequest() {
  expect(fixture.requests).toHaveLength(1);
  const [request] = fixture.requests;
  if (request === undefined) throw new Error("no request recorded");
  return request;
}

test("search GETs /libs/search with encoded libraryName and query", async () => {
  const libraries = { results: [{ id: "/tokio-rs/tokio", title: "Tokio" }] };
  fixture.plan([{ body: JSON.stringify(libraries) }]);
  const run = await context7(["search", "tokio", "async runtime"]);
  expect(run.status).toBe(0);
  expect(run.stdout).toBe(`${JSON.stringify(libraries, null, 2)}\n`);
  expect(fixture.connects).toEqual(["context7.com:443"]);
  const request = onlyRequest();
  expect(request.method).toBe("GET");
  expect(request.path).toBe("/api/v2/libs/search?libraryName=tokio&query=async%20runtime");
  expect(request.headers["accept"]).toBe("application/json");
});

test("search defaults the query to the library name, also as the bare command", async () => {
  const flagged = await context7(["search", "-l", "react"]);
  expect(flagged.status).toBe(0);
  expect(onlyRequest().path).toBe("/api/v2/libs/search?libraryName=react&query=react");
  fixture.plan([]);
  const bare = await context7(["next.js", "-q", "app router"]);
  expect(bare.status).toBe(0);
  expect(onlyRequest().path).toBe("/api/v2/libs/search?libraryName=next.js&query=app%20router");
});

test("a ctx7sk key is sent as a Bearer token", async () => {
  const run = await context7(["search", "react"], KEY);
  expect(run.status).toBe(0);
  expect(onlyRequest().headers["authorization"]).toBe(`Bearer ${KEY}`);
});

test("no key or a non-ctx7sk key sends no Authorization header", async () => {
  const none = await context7(["search", "react"], null);
  expect(none.status).toBe(0);
  expect(onlyRequest().headers["authorization"]).toBeUndefined();
  fixture.plan([]);
  const garbage = await context7(["search", "react"], "garbage-prefix-key");
  expect(garbage.status).toBe(0);
  expect(onlyRequest().headers["authorization"]).toBeUndefined();
});

test("docs --fast encodes the library id and appends fast=true", async () => {
  const run = await context7(["docs", "/vercel/next.js", "app router", "--fast"]);
  expect(run.status).toBe(0);
  expect(onlyRequest().path).toBe(
    "/api/v2/context?libraryId=%2Fvercel%2Fnext.js&query=app%20router&type=json&fast=true",
  );
});

test("docs without --fast sends no fast param", async () => {
  const run = await context7(["docs", "-l", "/vercel/next.js", "-q", "app router"]);
  expect(run.status).toBe(0);
  expect(onlyRequest().path).toBe(
    "/api/v2/context?libraryId=%2Fvercel%2Fnext.js&query=app%20router&type=json",
  );
});

test("docs -t txt prints the body unmodified", async () => {
  fixture.plan([{ body: "### Routing\nplain text, not JSON\n", contentType: "text/plain" }]);
  const run = await context7(["docs", "/vercel/next.js", "routing", "-t", "txt"]);
  expect(run.status).toBe(0);
  expect(run.stdout).toBe("### Routing\nplain text, not JSON\n");
  expect(onlyRequest().path).toContain("&type=txt");
});

test("docs needs both a library id and a query", async () => {
  const runs = await Promise.all([["docs"], ["docs", "/vercel/next.js"]].map((a) => context7(a)));
  for (const run of runs) {
    expect(run.status).toBe(1);
    expect(run.stderr).toBe(`${DOCS_USAGE}\n`);
    expect(run.stderr).toContain("--fast");
  }
  expect(fixture.connects).toHaveLength(0);
});

test("no command prints the banner; search without a library prints its usage", async () => {
  const [banner, help, search] = await Promise.all([
    context7([]),
    context7(["--help"]),
    context7(["search"]),
  ]);
  for (const run of [banner, help]) {
    expect(run.status).toBe(1);
    expect(run.stderr).toBe(`${MAIN_USAGE}\n`);
  }
  expect(search.status).toBe(1);
  expect(search.stderr).toBe(`${SEARCH_USAGE}\n`);
});

test("bad arguments exit 1 before any request", async () => {
  const cases: Array<[string[], string]> = [
    [["search", "a", "b", "c"], "Unknown option: c\n"],
    [["docs", "/x", "q", "extra"], "Unknown option: extra\n"],
    [["docs", "/x", "-q"], "context7: -q needs a value\n"],
  ];
  const runs = await Promise.all(cases.map(([args]) => context7(args)));
  runs.forEach((run, index) => {
    expect(run.status).toBe(1);
    expect(run.stderr).toBe(cases[index]?.[1] ?? "");
  });
  expect(fixture.connects).toHaveLength(0);
});

test("HTTP 429 exits 22 with the error body on stdout", async () => {
  fixture.plan([{ status: 429, body: '{"error":"rate limited"}' }]);
  const run = await context7(["search", "react"]);
  expect(run.status).toBe(22);
  expect(run.stdout).toBe('{\n  "error": "rate limited"\n}\n');
  expect(run.stderr).toBe(
    "context7: HTTP 429 from https://context7.com/api/v2/libs/search?libraryName=react&query=react\n",
  );
});

test("a non-JSON success on a JSON command exits 5", async () => {
  fixture.plan([{ body: "<html>", contentType: "text/html" }]);
  const run = await context7(["docs", "/x", "q"]);
  expect(run.status).toBe(5);
  expect(run.stderr).toBe("context7: response is not JSON\n");
});

test("a dropped connection exits 1 with a request-failed message", async () => {
  fixture.plan([{ close: true }]);
  const run = await context7(["search", "react"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toStartWith("context7: request failed: ");
});

test("long-form flags and an explicit --type json", async () => {
  const search = await context7(["search", "--library", "prisma", "--query", "relations"]);
  expect(search.status).toBe(0);
  expect(onlyRequest().path).toBe("/api/v2/libs/search?libraryName=prisma&query=relations");
  fixture.plan([]);
  const docs = await context7([
    "docs",
    "--library-id",
    "/prisma/prisma",
    "--query",
    "upsert",
    "--type",
    "json",
  ]);
  expect(docs.status).toBe(0);
  expect(onlyRequest().path).toBe(
    "/api/v2/context?libraryId=%2Fprisma%2Fprisma&query=upsert&type=json",
  );
});

test("any -t other than json prints the body raw", async () => {
  fixture.plan([{ body: "not json at all", contentType: "text/markdown" }]);
  const run = await context7(["docs", "/x", "q", "-t", "md"]);
  expect(run.status).toBe(0);
  expect(run.stdout).toBe("not json at all");
});

test("docs -t txt passes a JSON error body through raw, as cat did", async () => {
  fixture.plan([{ status: 404, body: '{"error":"not_found"}' }]);
  const run = await context7(["docs", "/x", "q", "-t", "txt"]);
  expect(run.status).toBe(22);
  expect(run.stdout).toBe('{"error":"not_found"}');
});

test("a library that names an Object.prototype member is still a library", async () => {
  const run = await context7(["search", "constructor"]);
  expect(run.status).toBe(0);
  expect(onlyRequest().path).toBe("/api/v2/libs/search?libraryName=constructor&query=constructor");
});

test("an empty success body prints nothing and exits 0, as jq did", async () => {
  fixture.plan([{ status: 204, body: "" }]);
  const run = await context7(["search", "react"]);
  expect(run.status).toBe(0);
  expect(run.stdout).toBe("");
});
