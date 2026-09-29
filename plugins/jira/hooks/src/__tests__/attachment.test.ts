/**
 * Attachments (ported from attachment.bats): multipart upload, list, metadata,
 * download (following Jira's redirect to its media host without forwarding
 * credentials), and read (text as context, binary as a saved path).
 */
import { afterAll, afterEach, beforeEach, expect, test } from "bun:test";
import {
  existsSync,
  mkdtempSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { BASE, fixtureBody, startJira } from "./harness.ts";

const h = await startJira();
afterAll(() => h.fixture.stop());

let sandbox = "";
beforeEach(() => {
  h.fixture.plan([]);
  sandbox = mkdtempSync(join(tmpdir(), "jira-attachment-"));
});
afterEach(() => rmSync(sandbox, { recursive: true, force: true }));

const API3 = `${BASE}/rest/api/3`;
const NOTES = fixtureBody("attachment-text-body.txt");

test("attachment: add uploads the file multipart to the issue endpoint", async () => {
  const file = join(sandbox, "up.txt");
  writeFileSync(file, "hi");
  const run = await h.jira(["attachment", "add", "ABC-1", file]);
  expect(run.status).toBe(0);
  const request = h.only();
  expect(request).toMatchObject({ method: "POST", url: `${API3}/issue/ABC-1/attachments` });
  expect(request.headers).toMatchObject({
    "x-atlassian-token": "no-check",
    authorization: "Bearer tok",
    accept: "application/json",
  });
  expect(request.headers["content-type"]).toStartWith("multipart/form-data; boundary=");
  expect(request.body).toContain('Content-Disposition: form-data; name="file"; filename="up.txt"');
  expect(request.body).toContain("\r\n\r\nhi\r\n");
});

test("attachment: add with a nonexistent file exits 1", async () => {
  const run = await h.jira(["attachment", "add", "ABC-1", join(sandbox, "missing.txt")]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("file not found");
  expect(h.fixture.requests).toHaveLength(0);
});

test("attachment: list hits the issue endpoint with the attachment field", async () => {
  h.fixture.plan([
    {
      body: '{"fields":{"attachment":[{"id":"10010","filename":"a.txt","size":3,"mimeType":"text/plain"}]}}',
    },
  ]);
  const run = await h.jira(["attachment", "list", "ABC-1", "--lean"]);
  expect(run.status).toBe(0);
  expect(h.only().url).toBe(`${API3}/issue/ABC-1?fields=attachment`);
  expect(JSON.parse(run.stdout)).toEqual({
    attachments: [{ id: "10010", filename: "a.txt", size: 3 }],
  });
});

test("attachment: get hits the attachment metadata endpoint", async () => {
  expect((await h.jira(["attachment", "get", "10010"])).status).toBe(0);
  expect(h.only().url).toBe(`${API3}/attachment/10010`);
});

test("attachment: download saves content with auth", async () => {
  h.fixture.plan([{ body: NOTES, contentType: "text/plain" }]);
  const out = join(sandbox, "out.bin");
  const run = await h.jira(["attachment", "download", "10010", "-o", out]);
  expect(run.status).toBe(0);
  expect(run.stdout).toBe(`downloaded attachment 10010 -> ${out}\n`);
  const request = h.only();
  expect(request.url).toBe(`${API3}/attachment/content/10010`);
  expect(request.headers).toMatchObject({ authorization: "Bearer tok", accept: "*/*" });
  expect(readFileSync(out, "utf8")).toBe(NOTES);
});

test("attachment: download follows the media redirect without forwarding credentials", async () => {
  h.fixture.plan([
    { status: 302, body: "", headers: { location: "https://media.example.net/file/10010" } },
    { body: NOTES, contentType: "text/plain" },
  ]);
  const out = join(sandbox, "out.txt");
  const run = await h.jira(["attachment", "download", "10010", "--output", out]);
  expect(run.status).toBe(0);
  expect(h.urls()).toEqual([
    `${API3}/attachment/content/10010`,
    "https://media.example.net/file/10010",
  ]);
  expect(h.fixture.requests[0]?.headers["authorization"]).toBe("Bearer tok");
  expect(h.fixture.requests[1]?.headers["authorization"]).toBeUndefined();
  expect(readFileSync(out, "utf8")).toBe(NOTES);
});

test("attachment: download without -o derives the filename from metadata", async () => {
  h.fixture.plan([{ body: fixtureBody("attachment-text.json") }, { body: NOTES }]);
  const run = await h.jira(["attachment", "download", "10010"], { cwd: sandbox });
  expect(run.status).toBe(0);
  expect(run.stdout).toBe("downloaded attachment 10010 -> deploy-notes.txt\n");
  expect(readFileSync(join(sandbox, "deploy-notes.txt"), "utf8")).toBe(NOTES);
});

test("attachment: a failed download exits 22 and writes no file", async () => {
  h.fixture.plan([{ status: 404, body: '{"errorMessages":["gone"]}' }]);
  const out = join(sandbox, "out.bin");
  const run = await h.jira(["attachment", "download", "10010", "-o", out]);
  expect(run.status).toBe(22);
  expect(run.stdout).toBe('{"errorMessages":["gone"]}');
  expect(existsSync(out)).toBe(false);
});

test("attachment: read prints text content as context", async () => {
  h.fixture.plan([{ body: fixtureBody("attachment-text.json") }, { body: NOTES }]);
  const run = await h.jira(["attachment", "read", "10010"]);
  expect(run.status).toBe(0);
  expect(run.stdout).toBe(`# deploy-notes.txt (text/plain)\n\n${NOTES}`);
});

test("attachment: read reports binary content instead of dumping bytes", async () => {
  h.fixture.plan([{ body: fixtureBody("attachment-image.json") }, { body: NOTES }]);
  const run = await h.jira(["attachment", "read", "10011"]);
  expect(run.status).toBe(0);
  const saved = /saved to (\S+)\n/.exec(run.stdout)?.[1];
  if (saved === undefined) throw new Error(`no saved path in: ${run.stdout}`);
  try {
    expect(run.stdout).toBe(
      `architecture.png — binary attachment (image/png, ${Buffer.byteLength(NOTES)} bytes) saved to ${saved}\n` +
        "Run `jira attachment download 10011 -o <path>` to save it elsewhere.\n",
    );
    expect(run.stdout).not.toContain("Deploy steps:");
    expect(readFileSync(saved, "utf8")).toBe(NOTES);
    // Owner-only, as mktemp created it: the temp dir may be shared.
    expect(statSync(saved).mode & 0o777).toBe(0o600);
  } finally {
    rmSync(saved, { force: true });
  }
});

test("attachment: unknown action exits 1 with usage", async () => {
  const run = await h.jira(["attachment", "bogus"]);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("Usage: jira attachment");
});

test("attachment: a metadata filename cannot steer the download outside the working directory", async () => {
  const meta = { id: "10010", filename: "../../escaped.txt", mimeType: "text/plain" };
  h.fixture.plan([{ body: JSON.stringify(meta) }, { body: NOTES }]);
  const work = join(sandbox, "a", "b");
  mkdirSync(work, { recursive: true });
  const run = await h.jira(["attachment", "download", "10010"], { cwd: work });
  expect(run.status).toBe(0);
  expect(run.stdout).toBe("downloaded attachment 10010 -> escaped.txt\n");
  expect(readdirSync(work)).toEqual(["escaped.txt"]);
  expect(existsSync(join(sandbox, "escaped.txt"))).toBe(false);
});

test("attachment: download without -o exits 22 when the metadata read fails", async () => {
  h.fixture.plan([{ status: 404, body: '{"errorMessages":["gone"]}' }]);
  const run = await h.jira(["attachment", "download", "10010"], { cwd: sandbox });
  expect(run.status).toBe(22);
  expect(run.stdout).toBe("");
  expect(run.stderr).toBe(`jira: HTTP 404 from ${API3}/attachment/10010\n`);
  expect(readdirSync(sandbox)).toEqual([]);
});

test("attachment: read exits 1 with no stdout when the content download fails", async () => {
  h.fixture.plan([
    { body: fixtureBody("attachment-text.json") },
    { status: 403, body: '{"errorMessages":["denied"]}' },
  ]);
  const run = await h.jira(["attachment", "read", "10010"]);
  expect(run.status).toBe(1);
  expect(run.stdout).toBe("");
  expect(run.stderr).toBe(`jira: HTTP 403 from ${API3}/attachment/content/10010\n`);
});
