/**
 * The committed exa-search bundle, run by path as the published symlink runs
 * it, against the loopback HTTPS fixture posing as api.exa.ai.
 */
import { afterAll, beforeEach, expect, test } from "bun:test";
import { startHttpsFixture } from "@toolu/conformance/https-fixture";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { symlinkSync } from "node:fs";
import { join } from "node:path";
import { CRAWL_USAGE, MAIN_USAGE, SEARCH_USAGE, SIMILAR_USAGE } from "../exa/usage.ts";

const BUNDLE = join(import.meta.dir, "../../dist/search.js");
const fixture = await startHttpsFixture(["api.exa.ai"]);

afterAll(() => fixture.stop());
beforeEach(() => fixture.plan([]));

async function exa(args: readonly string[], key: string | null = "k") {
  const env: Record<string, string | undefined> = { ...process.env, ...fixture.env };
  // Never inherit a developer's real key.
  delete env["EXA_API_KEY"];
  if (key !== null) env["EXA_API_KEY"] = key;
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
  const json: unknown = JSON.parse(request.body);
  return { ...request, json };
}

const EXA_RESPONSE = {
  requestId: "req-1",
  results: [
    {
      title: "Bun runtime",
      url: "https://bun.sh",
      image: "https://bun.sh/og.png",
      favicon: "https://bun.sh/favicon.ico",
      author: "",
      publishedDate: null,
      highlights: ["fast all-in-one toolkit"],
      subpages: [],
    },
  ],
};

test("search POSTs the bash-built body with x-api-key and prints jq-style JSON", async () => {
  fixture.plan([{ body: JSON.stringify(EXA_RESPONSE) }]);
  const run = await exa([
    "search",
    "-q",
    "needle",
    "--include-domains",
    "a.com,b.com",
    "--start-date",
    "2024-01-01",
    "--with-text",
  ]);
  expect(run.status).toBe(0);
  expect(run.stdout).toBe(`${JSON.stringify(EXA_RESPONSE, null, 2)}\n`);
  expect(fixture.connects).toEqual(["api.exa.ai:443"]);
  const request = onlyRequest();
  expect(request.method).toBe("POST");
  expect(request.path).toBe("/search");
  expect(request.headers).toMatchObject({
    "x-api-key": "k",
    "content-type": "application/json",
    accept: "application/json",
  });
  expect(request.json).toEqual({
    query: "needle",
    type: "auto",
    numResults: 10,
    contents: { highlights: { maxCharacters: 4000 }, text: true },
    includeDomains: ["a.com", "b.com"],
    startPublishedDate: "2024-01-01T00:00:00.000Z",
  });
});

test("every search option lands in the body", async () => {
  const run = await exa([
    "search",
    "--query",
    "q",
    "-n",
    "5",
    "-t",
    "fast",
    "-c",
    "research paper",
    "--exclude-domains",
    "x.com",
    "--end-date",
    "2024-12-31",
    "--include-text",
    "form",
    "--exclude-text",
    "deprecated",
    "--highlights",
    "100",
  ]);
  expect(run.status).toBe(0);
  expect(onlyRequest().json).toEqual({
    query: "q",
    type: "fast",
    numResults: 5,
    contents: { highlights: { maxCharacters: 100 } },
    category: "research paper",
    excludeDomains: ["x.com"],
    endPublishedDate: "2024-12-31T00:00:00.000Z",
    includeText: ["form"],
    excludeText: ["deprecated"],
  });
});

test("a bare query is the default command", async () => {
  const run = await exa(["rust async runtime"]);
  expect(run.status).toBe(0);
  const request = onlyRequest();
  expect(request.path).toBe("/search");
  expect(request.json).toMatchObject({ query: "rust async runtime" });
});

test("--lean keeps requestId and the non-empty prompt fields only", async () => {
  fixture.plan([{ body: JSON.stringify(EXA_RESPONSE) }]);
  const run = await exa(["search", "-q", "bun", "--lean"]);
  expect(run.status).toBe(0);
  const lean = {
    requestId: "req-1",
    results: [
      { title: "Bun runtime", url: "https://bun.sh", highlights: ["fast all-in-one toolkit"] },
    ],
  };
  expect(run.stdout).toBe(`${JSON.stringify(lean, null, 2)}\n`);
});

test("--lean tolerates a response with no results", async () => {
  const run = await exa(["search", "-q", "rust", "--lean"]);
  expect(run.status).toBe(0);
  expect(run.stdout).toBe('{\n  "requestId": null,\n  "results": []\n}\n');
});

test("crawl POSTs every url to /contents with the max-chars cap", async () => {
  const run = await exa(["crawl", "https://a.test/x", "https://b.test", "-m", "50"]);
  expect(run.status).toBe(0);
  const request = onlyRequest();
  expect(request.path).toBe("/contents");
  expect(request.json).toEqual({
    urls: ["https://a.test/x", "https://b.test"],
    text: true,
    highlights: { maxCharacters: 50 },
  });
});

test("similar POSTs to /findSimilar", async () => {
  const run = await exa(["similar", "https://blog.test/post", "-n", "3", "--highlights", "20"]);
  expect(run.status).toBe(0);
  const request = onlyRequest();
  expect(request.path).toBe("/findSimilar");
  expect(request.json).toEqual({
    url: "https://blog.test/post",
    numResults: 3,
    contents: { highlights: { maxCharacters: 20 } },
  });
});

test("a missing EXA_API_KEY exits 1 before any request", async () => {
  const runs = await Promise.all([null, ""].map((key) => exa(["search", "-q", "anything"], key)));
  for (const run of runs) {
    expect(run.status).toBe(1);
    expect(run.stderr).toBe("exa-search: EXA_API_KEY unset\n");
  }
  expect(fixture.connects).toHaveLength(0);
});

test("a project .env key does not satisfy the published executable's credential check", async () => {
  using sb = createSandbox();
  sb.write(".env", "EXA_API_KEY=dotenv-only-sentinel\n");
  const helper = sb.path("search.sh");
  symlinkSync(BUNDLE, helper);
  const result = await run([helper, "search", "-q", "needle"], {
    cwd: sb.project,
    env: { ...fixture.env, EXA_API_KEY: undefined },
  });
  expect(result.exitCode).toBe(1);
  expect(result.stderr).toBe("exa-search: EXA_API_KEY unset\n");
  expect(fixture.connects).toHaveLength(0);
});

test("no command prints the banner and exits 1", async () => {
  const runs = await Promise.all([[], ["-h"], ["--help"]].map((args) => exa(args)));
  for (const run of runs) {
    expect(run.status).toBe(1);
    expect(run.stderr).toBe(`${MAIN_USAGE}\n`);
  }
});

test("search usage advertises the spec-compliant type and category enums", async () => {
  const run = await exa(["search"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toBe(`${SEARCH_USAGE}\n`);
  expect(run.stderr).toContain("instant|fast|auto|deep-lite|deep|deep-reasoning");
  expect(run.stderr).toContain("company|research paper|news|personal site|financial report|people");
  expect(run.stderr).not.toContain("neural");
  expect(run.stderr).not.toContain("tweet");
});

test("crawl and similar without a url print their usage and exit 1", async () => {
  const crawl = await exa(["crawl"]);
  expect(crawl.status).toBe(1);
  expect(crawl.stderr).toBe(`${CRAWL_USAGE}\n`);
  const similar = await exa(["similar"]);
  expect(similar.status).toBe(1);
  expect(similar.stderr).toBe(`${SIMILAR_USAGE}\n`);
});

test("bad arguments exit before any request: 2 for a number jq would reject, else 1", async () => {
  const cases: Array<[string[], number, string]> = [
    [["search", "-q", "x", "-n", "abc"], 2, "exa-search: -n must be a number\n"],
    [
      ["search", "-q", "x", "--highlights", "0x10"],
      2,
      "exa-search: --highlights must be a number\n",
    ],
    [
      ["similar", "https://a.test", "--num-results", "ten"],
      2,
      "exa-search: --num-results must be a number\n",
    ],
    [
      ["crawl", "https://a.test", "--max-chars", "1k"],
      2,
      "exa-search: --max-chars must be a number\n",
    ],
    [["search", "-q"], 1, "exa-search: -q needs a value\n"],
    [["search", "-q", "x", "extra"], 1, "Unknown option: extra\n"],
    [["similar", "https://a.test", "https://b.test"], 1, "Unknown option: https://b.test\n"],
    [["crawl", "https://a.test", "-m"], 1, "exa-search: -m needs a value\n"],
  ];
  const runs = await Promise.all(cases.map(([args]) => exa(args)));
  runs.forEach((run, index) => {
    const [, status, stderr] = cases[index] ?? [[], -1, ""];
    expect(run.status).toBe(status);
    expect(run.stderr).toBe(stderr);
  });
  expect(fixture.connects).toHaveLength(0);
});

test("HTTP 401 exits 22 with the error body on stdout", async () => {
  fixture.plan([{ status: 401, body: '{"error":"bad"}' }]);
  const run = await exa(["search", "-q", "x"]);
  expect(run.status).toBe(22);
  expect(run.stdout).toBe('{\n  "error": "bad"\n}\n');
  expect(run.stderr).toBe("exa-search: HTTP 401 from https://api.exa.ai/search\n");
});

test("a non-JSON success exits 5", async () => {
  fixture.plan([{ body: "<html>", contentType: "text/html" }]);
  const run = await exa(["search", "-q", "x"]);
  expect(run.status).toBe(5);
  expect(run.stdout).toBe("");
  expect(run.stderr).toBe("exa-search: response is not JSON\n");
});

test("a dropped connection exits 1 with a request-failed message", async () => {
  fixture.plan([{ close: true }]);
  const run = await exa(["search", "-q", "x"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toStartWith("exa-search: request failed: ");
});

test("a bare query that names an Object.prototype member is still a query", async () => {
  const run = await exa(["constructor"]);
  expect(run.status).toBe(0);
  expect(onlyRequest().json).toMatchObject({ query: "constructor" });
});

test("an empty -q leaves the query to the next bare argument", async () => {
  const run = await exa(["search", "-q", "", "late query"]);
  expect(run.status).toBe(0);
  expect(onlyRequest().json).toMatchObject({ query: "late query" });
});

test("an empty --include-domains adds no domain filter", async () => {
  const run = await exa(["search", "-q", "x", "--include-domains", ""]);
  expect(run.status).toBe(0);
  expect(onlyRequest().json).not.toHaveProperty("includeDomains");
});

test("long-form crawl and similar flags", async () => {
  const crawl = await exa(["crawl", "https://a.test", "--max-chars", "70"]);
  expect(crawl.status).toBe(0);
  expect(onlyRequest().json).toMatchObject({ highlights: { maxCharacters: 70 } });
  fixture.plan([]);
  const similar = await exa(["similar", "https://a.test", "--num-results", "4"]);
  expect(similar.status).toBe(0);
  expect(onlyRequest().json).toMatchObject({ numResults: 4 });
});

test("an empty success body prints nothing and exits 0, as jq did", async () => {
  fixture.plan([{ status: 204, body: "" }]);
  const run = await exa(["search", "-q", "x", "--lean"]);
  expect(run.status).toBe(0);
  expect(run.stdout).toBe("");
});

test("--lean with an HTTP error prints the real error body, not an empty projection", async () => {
  fixture.plan([{ status: 401, body: '{"error":"bad"}' }]);
  const run = await exa(["search", "-q", "x", "--lean"]);
  expect(run.status).toBe(22);
  expect(run.stdout).toBe('{\n  "error": "bad"\n}\n');
});
