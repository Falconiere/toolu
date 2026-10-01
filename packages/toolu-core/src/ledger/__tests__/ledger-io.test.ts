/** Ledger file I/O and location on real files and repositories. */
import { describe, expect, test } from "bun:test";
import { mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { childEnv } from "@toolu/conformance/harness/spawn";
import { toJqJson } from "../../state/state-io.ts";
import type { Json } from "../ledger-jq.ts";
import { ledgerPath, readLedger, writeLedger } from "../ledger-io.ts";

const VALUES: Record<string, Json> = {
  ledger: { version: 1, branch: "feat/x", steps: [{ id: "s1", status: "green", n: 1.5 }] },
  "DEL and unicode": { s: "a\u007fb é ✓", empty: [], obj: {} },
  array: [1, "two", null],
};

describe("writeLedger", () => {
  for (const [name, value] of Object.entries(VALUES)) {
    test.concurrent(name, () => {
      using sb = createSandbox();
      const tsFile = sb.path("ts/nested/dir/l.json");
      expect(writeLedger(tsFile, value)).toBeUndefined();
      expect(readFileSync(tsFile, "utf8")).toBe(`${toJqJson(value, true)}\n`);
      expect(readdirSync(dirname(tsFile))).toEqual(["l.json"]);
    });
  }

  test.concurrent("a file blocking the parent directory returns a tagged error", () => {
    using sb = createSandbox();
    sb.write("blocker", "a file where a directory should be");
    const file = sb.path("blocker/l.json");
    expect(writeLedger(file, {})).toBe(
      `plan-ledger-parse: cannot create ledger dir: ${sb.path("blocker")}`,
    );
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

describe("readLedger", () => {
  for (const [name, body] of Object.entries(FILES)) {
    test.concurrent(name, () => {
      using sb = createSandbox();
      const file = sb.path("l.json");
      if (body === "<dir>") mkdirSync(join(file, "x"), { recursive: true });
      else if (body !== null) writeFileSync(file, body);
      const got = readLedger(file);
      if (
        body !== null &&
        body !== "<dir>" &&
        body !== "" &&
        body !== "null\n" &&
        body !== "false" &&
        body !== "{not json"
      ) {
        expect(got?.text).toBe(body.replace(/\n+$/, ""));
      } else {
        expect(got).toBeUndefined();
      }
    });
  }
});

describe("ledgerPath", () => {
  const cases: Record<string, { branch?: string; host: string; unborn?: boolean; repo?: boolean }> =
    {
      "claude feature branch": { branch: "feat/256-x.y", host: "claude" },
      "codex host": { branch: "fix/a", host: "codex" },
      "unborn HEAD": { host: "claude", unborn: true },
      "not a repo": { host: "claude", repo: false },
    };
  for (const [name, c] of Object.entries(cases)) {
    test.concurrent(name, () => {
      using sb = createSandbox(c.repo === false ? {} : { git: c.unborn !== true });
      if (c.unborn === true) sb.git("init", "-q", "-b", "main");
      if (c.branch !== undefined) sb.git("checkout", "-q", "-b", c.branch);
      mkdirSync(sb.path("sub/dir"), { recursive: true });
      const cwd = sb.path("sub/dir");
      const env = { TOOLU_HOST_OVERRIDE: c.host, HOME: sb.home };
      const got = ledgerPath({ cwd, env: childEnv(env) });
      if (c.repo === false || c.unborn === true) expect(got).toBeUndefined();
      else
        expect(got).toBe(
          sb.path(
            `${c.host === "codex" ? ".codex" : ".claude"}/tmp/plan-ledger/${c.branch?.replaceAll("/", "_").replaceAll(".", "")}.json`,
          ),
        );
    });
  }
});
