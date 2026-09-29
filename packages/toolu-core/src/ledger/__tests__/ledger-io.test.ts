/**
 * Ledger file I/O and location (#256; AC-3, AC-4) against `pl_read_ledger`,
 * `pl_write_ledger` and `pl_ledger_path`. The file bytes, mode, read verdict
 * and resolved path must match bash on real files and real repos.
 */
import { describe, expect, test } from "bun:test";
import { mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { childEnv } from "@toolu/conformance/harness/spawn";
import type { Json } from "../ledger-jq.ts";
import { ledgerPath, readLedger, writeLedger } from "../ledger-io.ts";
import { bashFn } from "./ledger-parity-helpers.ts";

const PARSE = "plan-ledger-parse.sh";

const VALUES: Record<string, Json> = {
  ledger: { version: 1, branch: "feat/x", steps: [{ id: "s1", status: "green", n: 1.5 }] },
  "DEL and unicode": { s: "a\u007fb é ✓", empty: [], obj: {} },
  array: [1, "two", null],
};

describe("writeLedger vs pl_write_ledger", () => {
  for (const [name, value] of Object.entries(VALUES)) {
    test.concurrent(name, async () => {
      using sb = createSandbox();
      const bashFile = sb.path("bash/nested/dir/l.json");
      const tsFile = sb.path("ts/nested/dir/l.json");
      const res = await bashFn(PARSE, "pl_write_ledger", [bashFile, JSON.stringify(value)], {
        cwd: sb.project,
      });
      expect(res.exitCode).toBe(0);
      expect(writeLedger(tsFile, value)).toBeUndefined();
      expect(readFileSync(tsFile, "utf8")).toBe(readFileSync(bashFile, "utf8"));
      expect(statSync(tsFile).mode).toBe(statSync(bashFile).mode);
      expect(readdirSync(dirname(tsFile))).toEqual(["l.json"]);
    });
  }

  test.concurrent("an unwritable directory fails with the same tagged line", async () => {
    using sb = createSandbox();
    sb.write("blocker", "a file where a directory should be");
    const file = sb.path("blocker/l.json");
    const res = await bashFn(PARSE, "pl_write_ledger", [file, "{}"], { cwd: sb.project });
    expect(res.exitCode).not.toBe(0);
    const line = res.stderr.split("\n").find((l) => l.startsWith("plan-ledger-parse:"));
    expect(writeLedger(file, {})).toBe(line);
  });
});

/** name → file body; `null` leaves the path absent, `"<dir>"` makes it a directory. */
const FILES: Record<string, string | null> = {
  absent: null,
  empty: "",
  "json null": "null\n",
  "json false": "false",
  "json zero": "0",
  garbage: "{not json",
  "compact object": '{"version":1,"steps":[]}',
  "trailing newlines": '{"a":1}\n\n\n',
  "empty array": "[]",
  directory: "<dir>",
};

describe("readLedger vs pl_read_ledger", () => {
  for (const [name, body] of Object.entries(FILES)) {
    test.concurrent(name, async () => {
      using sb = createSandbox();
      const file = sb.path("l.json");
      if (body === "<dir>") mkdirSync(join(file, "x"), { recursive: true });
      else if (body !== null) writeFileSync(file, body);
      const res = await bashFn(PARSE, "pl_read_ledger", [file], { cwd: sb.project });
      const got = readLedger(file);
      if (res.exitCode === 0) {
        expect(got?.text).toBe(res.stdout.replace(/\n$/, ""));
      } else {
        expect(got).toBeUndefined();
      }
    });
  }
});

describe("ledgerPath vs pl_ledger_path", () => {
  const cases: Record<string, { branch?: string; host: string; unborn?: boolean; repo?: boolean }> =
    {
      "claude feature branch": { branch: "feat/256-x.y", host: "claude" },
      "codex host": { branch: "fix/a", host: "codex" },
      "unborn HEAD": { host: "claude", unborn: true },
      "not a repo": { host: "claude", repo: false },
    };
  for (const [name, c] of Object.entries(cases)) {
    test.concurrent(name, async () => {
      using sb = createSandbox(c.repo === false ? {} : { git: c.unborn !== true });
      if (c.unborn === true) sb.git("init", "-q", "-b", "main");
      if (c.branch !== undefined) sb.git("checkout", "-q", "-b", c.branch);
      mkdirSync(sb.path("sub/dir"), { recursive: true });
      const cwd = sb.path("sub/dir");
      const env = { TOOLU_HOST_OVERRIDE: c.host, HOME: sb.home };
      const res = await bashFn("plan-ledger.sh", "pl_ledger_path", [], { cwd, env });
      const got = ledgerPath({ cwd, env: childEnv(env) });
      expect(got === undefined ? "" : `${got}\n`).toBe(res.stdout);
      expect(got === undefined).toBe(res.exitCode !== 0);
    });
  }
});
