import { afterAll, expect, test } from "bun:test";
import { CliExit } from "../../cli/cli.ts";
import { download, encodeQuery, formatJson, jsonOutput, parseJson, send } from "../rest.ts";

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
    if (url.pathname === "/empty") return new Response(null, { status: 204 });
    if (url.pathname === "/bytes") return new Response(new Uint8Array([0, 1, 254, 255]));
    if (url.pathname === "/hop") {
      const to = url.searchParams.get("to") ?? "/bytes";
      return new Response(null, { status: 302, headers: { location: to } });
    }
    if (url.pathname === "/loop") {
      return new Response(null, { status: 302, headers: { location: "/loop" } });
    }
    if (url.pathname === "/moved") {
      return new Response('{"moved":true}', {
        status: 302,
        headers: { location: `${url.origin}/elsewhere` },
      });
    }
    return Response.json({ ok: true });
  },
});
const base = `http://127.0.0.1:${server.port}`;

// A second origin (another port): where a cross-origin redirect lands.
const media: Seen[] = [];
const mediaServer = Bun.serve({
  hostname: "127.0.0.1",
  port: 0,
  async fetch(request) {
    const url = new URL(request.url);
    media.push({
      method: request.method,
      path: url.pathname,
      headers: request.headers.toJSON(),
      body: await request.text(),
    });
    return new Response("media bytes");
  },
});
const mediaBase = `http://127.0.0.1:${mediaServer.port}`;

afterAll(async () => {
  await server.stop(true);
  await mediaServer.stop(true);
});

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

test("redirects are not followed, so a key header never reaches another location", async () => {
  seen.length = 0;
  const text = await send("tool", { url: `${base}/moved`, headers: { "x-api-key": "k" } });
  expect(text).toBe('{"moved":true}');
  expect(seen.map((request) => request.path)).toEqual(["/moved"]);
});

test("json: false passes an error body through unformatted", async () => {
  const exit = await rejected(send("tool", { url: `${base}/denied`, json: false }));
  expect(exit.code).toBe(22);
  expect(exit.stdout).toBe('{"error":"bad"}');
});

test("jsonOutput prints nothing for an empty body and projects a JSON one", async () => {
  expect(jsonOutput("tool", await send("tool", { url: `${base}/empty` }))).toBe("");
  expect(jsonOutput("tool", '{"a":1}', () => ({ b: 2 }))).toBe('{\n  "b": 2\n}\n');
  expect(jsonOutput("tool", "[1]")).toBe("[\n  1\n]\n");
});

test("send PUTs and DELETEs, and a string payload goes out verbatim", async () => {
  await send("tool", { url: `${base}/put`, method: "PUT", payload: '{"a": 1 }' });
  expect(seen.at(-1)).toMatchObject({ method: "PUT", path: "/put", body: '{"a": 1 }' });
  await send("tool", { url: `${base}/gone`, method: "DELETE" });
  expect(seen.at(-1)).toMatchObject({ method: "DELETE", path: "/gone", body: "" });
});

test("a FormData payload goes out as multipart with the file name and bytes", async () => {
  const form = new FormData();
  form.append("file", new Blob(["hi"], { type: "text/plain" }), "up.txt");
  await send("tool", { url: `${base}/upload`, method: "POST", payload: form });
  const request = seen.at(-1);
  expect(request?.headers["content-type"]).toStartWith("multipart/form-data; boundary=");
  expect(request?.body).toContain('Content-Disposition: form-data; name="file"; filename="up.txt"');
  expect(request?.body).toContain("\r\n\r\nhi\r\n");
});

test("download returns the raw bytes", async () => {
  const bytes = await download("tool", { url: `${base}/bytes` });
  expect([...bytes]).toEqual([0, 1, 254, 255]);
});

test("download follows a same-origin redirect and keeps Authorization", async () => {
  seen.length = 0;
  const bytes = await download("tool", {
    url: `${base}/hop?to=/bytes`,
    headers: { Authorization: "Bearer tok", Accept: "*/*" },
  });
  expect(bytes.length).toBe(4);
  expect(seen.map((request) => [request.path, request.headers["authorization"]])).toEqual([
    ["/hop?to=/bytes", "Bearer tok"],
    ["/bytes", "Bearer tok"],
  ]);
});

test("download follows a cross-origin redirect without forwarding Authorization", async () => {
  media.length = 0;
  const to = encodeURIComponent(`${mediaBase}/file`);
  const bytes = await download("tool", {
    url: `${base}/hop?to=${to}`,
    headers: { Authorization: "Bearer tok", Accept: "*/*" },
  });
  expect(new TextDecoder().decode(bytes)).toBe("media bytes");
  expect(media).toHaveLength(1);
  expect(media[0]?.headers["authorization"]).toBeUndefined();
  expect(media[0]?.headers["accept"]).toBe("*/*");
});

test("download exits 22 with the raw error body on HTTP >= 400", async () => {
  const exit = await rejected(download("tool", { url: `${base}/denied` }));
  expect(exit.code).toBe(22);
  expect(exit.message).toBe(`tool: HTTP 401 from ${base}/denied`);
  expect(exit.stdout).toBe('{"error":"bad"}');
});

test("download exits 47, as curl did, after too many redirects", async () => {
  const exit = await rejected(download("tool", { url: `${base}/loop` }));
  expect(exit.code).toBe(47);
  expect(exit.message).toStartWith("tool: too many redirects from ");
});
