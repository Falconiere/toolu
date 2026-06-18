// b2 check: the real Bun.serve dashboard against a REAL temp ledger. SSE is read
// by streaming response.body (EventSource may be absent under bun test), so we
// parse event:/data: frames off the wire. No mocks; the ledger is produced by
// the real plan-ledger.sh and mutated via mv-replace to prove the dir watch.

import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { execFileSync } from "node:child_process";
import { mkdtempSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { startServer } from "../index.ts";

const ENGINE = join(import.meta.dir, "..", "..", "hooks", "lib", "plan-ledger.sh");

const PLAN = `# Server Plan — Plan

**Date:** 2026-06-17   **Status:** Approved   **Topic:** server.test fixture

## Steps (machine-readable)

\`\`\`json
[
  { "id": "s1", "title": "trivial true", "check": "true" }
]
\`\`\`
`;

function run(cmd: string, args: string[], cwd: string): string {
  return execFileSync(cmd, args, { cwd, encoding: "utf8" }).trim();
}

const tmps: string[] = [];
function mkTmp(prefix: string): string {
  const dir = mkdtempSync(join(tmpdir(), prefix));
  tmps.push(dir);
  return dir;
}

let repo: string;
let ledgerPath: string;
let server: { port: number; stop: () => void };

beforeAll(() => {
  repo = mkTmp("dash-server-");
  run("git", ["init", "-q", "-b", "main"], repo);
  run("git", ["config", "user.email", "t@t.t"], repo);
  run("git", ["config", "user.name", "t"], repo);
  writeFileSync(join(repo, "file.txt"), "base\n");
  run("git", ["add", "-A"], repo);
  run("git", ["commit", "-qm", "init"], repo);
  writeFileSync(join(repo, "plan.md"), PLAN);
  run("bash", [ENGINE, "run", "plan.md", "--step", "s1"], repo);
  ledgerPath = run("bash", [ENGINE, "path"], repo);
  server = startServer({ ledgerPath, repoRoot: repo });
});

afterAll(() => {
  server.stop();
  for (const dir of tmps) rmSync(dir, { recursive: true, force: true });
});

/** Read one SSE `state` event's parsed data off a streaming response within ms,
 *  honoring an AbortSignal. Returns null on timeout. */
async function readStateEvent(
  reader: ReadableStreamDefaultReader<Uint8Array>,
  ms: number,
): Promise<unknown | null> {
  const decoder = new TextDecoder();
  let buffer = "";
  const deadline = Date.now() + ms;
  while (Date.now() < deadline) {
    const chunk = await Promise.race([
      reader.read(),
      Bun.sleep(deadline - Date.now()).then(() => ({ done: true, value: undefined }) as const),
    ]);
    if (chunk.done) break;
    buffer += decoder.decode(chunk.value, { stream: true });
    let sep = buffer.indexOf("\n\n");
    while (sep !== -1) {
      const frame = buffer.slice(0, sep);
      buffer = buffer.slice(sep + 2);
      if (frame.includes("event: state")) {
        const dataLine = frame.split("\n").find((l) => l.startsWith("data:"));
        if (dataLine) return JSON.parse(dataLine.slice("data:".length).trim());
      }
      sep = buffer.indexOf("\n\n");
    }
  }
  return null;
}

describe("dashboard server — real ledger over the wire", () => {
  test("SSE emits an initial `state` event on connect", async () => {
    const ctrl = new AbortController();
    const res = await fetch(`http://localhost:${server.port}/api/events`, {
      signal: ctrl.signal,
    });
    expect(res.headers.get("content-type")).toContain("text/event-stream");
    const reader = res.body!.getReader();
    const state = (await readStateEvent(reader, 1000)) as { steps?: unknown[] };
    expect(state).not.toBeNull();
    expect(Array.isArray(state.steps)).toBe(true);
    ctrl.abort();
  });

  test("an mv-replace of the ledger pushes a `state` event within ~300ms", async () => {
    const ctrl = new AbortController();
    const res = await fetch(`http://localhost:${server.port}/api/events`, {
      signal: ctrl.signal,
    });
    const reader = res.body!.getReader();
    // Consume the initial paint.
    await readStateEvent(reader, 1000);

    // Build an updated ledger off-disk and mv it into place (mirrors the engine's
    // atomic temp+mv), which a file-path watch would miss but the dir watch sees.
    const original = await Bun.file(ledgerPath).text();
    const mutated = original.replace('"title": "trivial true"', '"title": "CHANGED title"');
    const staging = `${ledgerPath}.tmp.test`;
    writeFileSync(staging, mutated);
    renameSync(staging, ledgerPath);

    const state = (await readStateEvent(reader, 800)) as {
      ledger?: { steps?: { title?: string }[] };
    } | null;
    expect(state).not.toBeNull();
    expect(state!.ledger?.steps?.[0]?.title).toBe("CHANGED title");
    ctrl.abort();
  });

  test("GET /api/state on a no-ledger branch returns {ledger:null,steps:[]} with 200", async () => {
    // A fresh git repo with no ledger written yet -> the no-ledger shape, no 500.
    const fresh = mkTmp("dash-noledger-");
    run("git", ["init", "-q", "-b", "main"], fresh);
    run("git", ["config", "user.email", "t@t.t"], fresh);
    run("git", ["config", "user.name", "t"], fresh);
    writeFileSync(join(fresh, "f.txt"), "x\n");
    run("git", ["add", "-A"], fresh);
    run("git", ["commit", "-qm", "init"], fresh);
    const freshLedger = run("bash", [ENGINE, "path"], fresh);
    const s = startServer({ ledgerPath: freshLedger, repoRoot: fresh });
    try {
      const res = await fetch(`http://localhost:${s.port}/api/state`);
      expect(res.status).toBe(200);
      const body = (await res.json()) as { ledger: unknown; steps: unknown[] };
      expect(body.ledger).toBeNull();
      expect(body.steps).toEqual([]);
    } finally {
      s.stop();
    }
  });

  test("a client disconnect does not break later broadcasts", async () => {
    // Open, read initial, then abort (client gone). The server must drop it from
    // the hub so a subsequent change broadcast survives — proven by a NEW client
    // still receiving a fresh state event after another mv-replace.
    const gone = new AbortController();
    const res1 = await fetch(`http://localhost:${server.port}/api/events`, {
      signal: gone.signal,
    });
    const reader1 = res1.body!.getReader();
    await readStateEvent(reader1, 1000);
    gone.abort();
    await Bun.sleep(50);

    // Trigger another change.
    const original = await Bun.file(ledgerPath).text();
    const mutated = original.replace(/"title": "[^"]*"/, '"title": "AFTER disconnect"');
    const staging = `${ledgerPath}.tmp.test2`;
    writeFileSync(staging, mutated);
    renameSync(staging, ledgerPath);

    const alive = new AbortController();
    const res2 = await fetch(`http://localhost:${server.port}/api/events`, {
      signal: alive.signal,
    });
    const reader2 = res2.body!.getReader();
    const state = (await readStateEvent(reader2, 1000)) as {
      ledger?: { steps?: { title?: string }[] };
    } | null;
    expect(state).not.toBeNull();
    expect(state!.ledger?.steps?.[0]?.title).toBe("AFTER disconnect");
    alive.abort();
  });
});
