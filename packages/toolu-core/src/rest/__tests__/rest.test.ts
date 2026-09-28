import { afterAll, expect, test } from "bun:test";
import { CliExit } from "../../cli/cli.ts";
import { encodeQuery, formatJson, parseJson, send } from "../rest.ts";

interface Seen {
  method: string;
  path: string;
  headers: Record<string, string>;
  body: string;
}

const seen: Seen[] = [];

// A real loopback server: each path picks the response the CLI must handle.
const server = Bun.serve({
  hostname: "127.0.0.1",
  port: 0,
  async fetch(request) {
    const url = new URL(request.url);
    seen.push({
      method: request.method,
      path: url.pathname + url.search,
      headers: request.headers.toJSON(),
      body: await request.text(),
    });
    if (url.pathname === "/denied") return Response.json({ error: "bad" }, { status: 401 });
    if (url.pathname === "/html-error") return new Response("<html>", { status: 502 });
    if (url.pathname === "/html") return new Response("<html>");
    return Response.json({ ok: true });
  },
});
const base = `http://127.0.0.1:${server.port}`;

afterAll(() => server.stop(true));

async function rejected(promise: Promise<unknown>): Promise<CliExit> {
  try {
    await promise;
  } catch (error) {
    if (error instanceof CliExit) return error;
    throw error;
  }
  throw new Error("expected a CliExit");
}

test("send POSTs a JSON body with the given headers and returns the text", async () => {
  const text = await send("tool", {
    url: `${base}/search`,
    method: "POST",
    headers: { "x-api-key": "k" },
    body: { query: "needle" },
  });
  expect(JSON.parse(text)).toEqual({ ok: true });
  const request = seen.at(-1);
  expect(request?.method).toBe("POST");
  expect(request?.headers["x-api-key"]).toBe("k");
  expect(JSON.parse(request?.body ?? "")).toEqual({ query: "needle" });
});

test("send GETs without a body by default", async () => {
  await send("tool", { url: `${base}/libs?x=1` });
  expect(seen.at(-1)).toMatchObject({ method: "GET", path: "/libs?x=1", body: "" });
});

test("HTTP >= 400 exits 22 with the pretty JSON body for stdout", async () => {
  const exit = await rejected(send("tool", { url: `${base}/denied` }));
  expect(exit.code).toBe(22);
  expect(exit.message).toBe(`tool: HTTP 401 from ${base}/denied`);
  expect(exit.stdout).toBe('{\n  "error": "bad"\n}\n');
});

test("HTTP >= 400 with a non-JSON body keeps the body raw", async () => {
  const exit = await rejected(send("tool", { url: `${base}/html-error` }));
  expect(exit.code).toBe(22);
  expect(exit.stdout).toBe("<html>");
});

test("a refused connection exits 1 with a request-failed message", async () => {
  const closed = Bun.serve({ hostname: "127.0.0.1", port: 0, fetch: () => new Response() });
  const url = `http://127.0.0.1:${closed.port}/`;
  await closed.stop(true);
  const exit = await rejected(send("tool", { url }));
  expect(exit.code).toBe(1);
  expect(exit.message).toStartWith("tool: request failed: ");
});

test("parseJson exits 5 on a non-JSON success body", async () => {
  const text = await send("tool", { url: `${base}/html` });
  let exit: unknown;
  try {
    parseJson("tool", text);
  } catch (error) {
    exit = error;
  }
  expect(exit).toBeInstanceOf(CliExit);
  expect(exit).toMatchObject({ code: 5, message: "tool: response is not JSON" });
});

test("formatJson matches jq '.' two-space layout", () => {
  expect(formatJson({ a: [1, { b: null }], c: [], d: {} })).toBe(
    '{\n  "a": [\n    1,\n    {\n      "b": null\n    }\n  ],\n  "c": [],\n  "d": {}\n}\n',
  );
});

test("encodeQuery percent-encodes everything but RFC 3986 unreserved characters", () => {
  expect(
    encodeQuery([
      ["libraryId", "/vercel/next.js"],
      ["query", "async runtime"],
    ]),
  ).toBe("?libraryId=%2Fvercel%2Fnext.js&query=async%20runtime");
  expect(encodeQuery([["q", "a!b'c(d)e*f~g_h-i.j"]])).toBe("?q=a%21b%27c%28d%29e%2Af~g_h-i.j");
  expect(encodeQuery([["q", "é"]])).toBe("?q=%C3%A9");
  expect(encodeQuery([])).toBe("");
});
