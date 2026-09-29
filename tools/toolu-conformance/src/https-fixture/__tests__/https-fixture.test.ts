import { afterAll, beforeEach, expect, test } from "bun:test";
import { mkdtempSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { startHttpsFixture } from "../https-fixture.ts";

const fixture = await startHttpsFixture(["api.example.test", "docs.example.test"]);

afterAll(() => fixture.stop());
beforeEach(() => fixture.plan([]));

// A real client in a separate bun process: fetch with the fixture's env applied.
const CLIENT = `
const response = await fetch(process.argv[1], { method: "POST", headers: { "x-key": "k" }, body: "{\\"a\\":1}" });
process.stdout.write(response.status + " " + (await response.text()));
`;

// Async spawn: a sync one would block the event loop the fixture serves on.
async function fetchVia(url: string, env: Readonly<Record<string, string>>) {
  const child = Bun.spawn([process.execPath, "-e", CLIENT, url], {
    env: { ...process.env, ...env },
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, status] = await Promise.all([new Response(child.stdout).text(), child.exited]);
  return { status, stdout };
}

test("a real https URL reaches the fixture through the proxy and is recorded", async () => {
  fixture.plan([{ status: 201, body: '{"ok":true}' }]);
  const run = await fetchVia("https://api.example.test/v1/search?q=a%20b", fixture.env);
  expect(run.stdout).toBe('201 {"ok":true}');
  expect(fixture.connects).toEqual(["api.example.test:443"]);
  expect(fixture.requests).toHaveLength(1);
  expect(fixture.requests[0]).toMatchObject({
    method: "POST",
    path: "/v1/search?q=a%20b",
    body: '{"a":1}',
  });
  expect(fixture.requests[0]?.headers).toMatchObject({ host: "api.example.test", "x-key": "k" });
});

test("the response plan advances and repeats its last entry", async () => {
  fixture.plan([{ status: 500, body: "first" }, { body: "rest" }]);
  // Sequential on purpose: the plan is consumed in arrival order.
  const url = "https://docs.example.test/";
  const first = await fetchVia(url, fixture.env);
  const second = await fetchVia(url, fixture.env);
  const third = await fetchVia(url, fixture.env);
  expect([first.stdout, second.stdout, third.stdout]).toEqual([
    "500 first",
    "200 rest",
    "200 rest",
  ]);
});

test("an unplanned request gets 200 {}", async () => {
  expect((await fetchVia("https://api.example.test/", fixture.env)).stdout).toBe("200 {}");
});

test("close drops the tunnel so the client sees a transport failure", async () => {
  fixture.plan([{ close: true }]);
  const run = await fetchVia("https://api.example.test/", fixture.env);
  expect(run.status).not.toBe(0);
  expect(fixture.connects).toEqual(["api.example.test:443"]);
  expect(fixture.requests).toHaveLength(0);
});

test("TLS verification stays on: without the fixture CA the client fails", async () => {
  const run = await fetchVia("https://api.example.test/", {
    ...fixture.env,
    NODE_EXTRA_CA_CERTS: "",
  });
  expect(run.status).not.toBe(0);
  expect(fixture.requests).toHaveLength(0);
});

/**
 * Puts a variable back as it was. Assigning `undefined` to process.env stores the
 * string "undefined", which would leak into every later test in the process
 * (CI runners leave TMPDIR unset).
 */
function restoreEnv(name: string, value: string | undefined): void {
  if (value === undefined) delete process.env[name];
  else process.env[name] = value;
}

function entries(dir: string): string[] {
  return readdirSync(dir);
}

test("a failed start (no openssl on PATH) throws and leaves no temp dir behind", async () => {
  // A private TMPDIR keeps the leftover check hermetic from other fixtures.
  const privateTmp = mkdtempSync(join(tmpdir(), "https-fixture-test-"));
  const saved = { PATH: process.env["PATH"], TMPDIR: process.env["TMPDIR"] };
  process.env["PATH"] = "/nonexistent";
  process.env["TMPDIR"] = privateTmp;
  // The fixture's temp dir must land in privateTmp, or the leftover check below
  // would pass vacuously: prove tmpdir() follows the TMPDIR just set.
  const followed = tmpdir();
  let failure: unknown;
  try {
    await startHttpsFixture(["api.example.test"]);
  } catch (error) {
    failure = error;
  } finally {
    restoreEnv("PATH", saved.PATH);
    restoreEnv("TMPDIR", saved.TMPDIR);
  }
  expect(followed).toBe(privateTmp);
  expect(failure).toBeInstanceOf(Error);
  expect(String(failure)).toContain("openssl failed");
  expect(entries(privateTmp)).toEqual([]);
  rmSync(privateTmp, { recursive: true, force: true });
});
