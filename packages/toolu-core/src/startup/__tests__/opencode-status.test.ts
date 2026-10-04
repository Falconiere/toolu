/**
 * The OpenCode status record reader (#359): real files on disk, a strict
 * shape, and missing versus invalid told apart so the report can say which.
 */
import { expect, test } from "bun:test";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import {
  MAX_OPENCODE_STATUS_BYTES,
  opencodeStatusPath,
  readOpencodeStatus,
  type OpencodeStatusRecord,
} from "../startup.ts";

const READY: OpencodeStatusRecord = {
  version: 1,
  written: "2026-10-04T12:00:00.000Z",
  project: "/work/project",
  status: "ready",
  selection: "project",
  plugins: [
    { name: "statusline", entries: ["session-start"], artifacts: 1 },
    { name: "jev", entries: ["session-start"], artifacts: 1 },
  ],
  notes: ['enabled plugin "nope" in /work/project/.opencode/toolu/plugins.json is not installed'],
};

const NOT_READY: OpencodeStatusRecord = {
  version: 1,
  written: "2026-10-04T12:00:00.000Z",
  project: "/work/project",
  status: "not-ready",
  reason: "bootstrap: jev/session-start: entry missing",
  plugins: [],
  notes: [],
};

function writeRecord(root: string, text: string): string {
  const path = opencodeStatusPath(root);
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, text);
  return path;
}

test.concurrent("the record lives under the config root's toolu directory", () => {
  expect(opencodeStatusPath("/data/root")).toBe("/data/root/toolu/opencode-status.json");
});

test.concurrent("ready and not-ready records read back exactly", () => {
  using sb = createSandbox();
  for (const record of [READY, NOT_READY]) {
    const path = writeRecord(sb.path(record.status), `${JSON.stringify(record)}\n`);
    expect(readOpencodeStatus(path)).toEqual({ ok: true, record });
  }
});

test.concurrent("an absent record, or one under a file, is missing", () => {
  using sb = createSandbox();
  expect(readOpencodeStatus(sb.path("absent/toolu/opencode-status.json"))).toEqual({
    ok: false,
    reason: "missing",
  });
  writeFileSync(sb.path("file"), "x");
  expect(readOpencodeStatus(join(sb.path("file"), "toolu/opencode-status.json"))).toEqual({
    ok: false,
    reason: "missing",
  });
});

test.concurrent("malformed, partial, foreign and oversized records are invalid", () => {
  using sb = createSandbox();
  const full = JSON.stringify(READY);
  const cases: Record<string, string> = {
    empty: "",
    object: "{}",
    truncated: full.slice(0, full.length - 9),
    version: JSON.stringify({ ...READY, version: 2 }),
    extraKey: JSON.stringify({ ...READY, token: "x" }),
    status: JSON.stringify({ ...READY, status: "ok" }),
    selection: JSON.stringify({ ...READY, selection: "user" }),
    pluginKey: JSON.stringify({
      ...READY,
      plugins: [{ name: "jev", entries: [], artifacts: 1, x: 1 }],
    }),
    artifacts: JSON.stringify({ ...READY, plugins: [{ name: "jev", entries: [], artifacts: -1 }] }),
    notes: JSON.stringify({ ...READY, notes: [1] }),
    array: "[]",
    oversized: JSON.stringify({ ...READY, notes: ["x".repeat(MAX_OPENCODE_STATUS_BYTES)] }),
  };
  for (const [name, text] of Object.entries(cases)) {
    const path = writeRecord(sb.path(name), text);
    expect({ name, read: readOpencodeStatus(path) }).toEqual({
      name,
      read: { ok: false, reason: "invalid" },
    });
  }
});

test.concurrent("a directory at the record path is invalid, not missing", () => {
  using sb = createSandbox();
  const path = opencodeStatusPath(sb.path("dir"));
  mkdirSync(path, { recursive: true });
  expect(readOpencodeStatus(path)).toEqual({ ok: false, reason: "invalid" });
});
