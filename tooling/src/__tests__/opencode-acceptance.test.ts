import { expect, test } from "bun:test";
import { join } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { parseAcceptanceArgs } from "../opencode-acceptance.ts";

// `bun run test:opencode` arguments (#362): narrowing is explicit, and a typo fails before any host run.

const ROOT = join(import.meta.dir, "../../..");

test.concurrent("no arguments run everything and report under the temp dir", () => {
  const args = parseAcceptanceArgs([]);
  expect(args.only).toEqual([]);
  expect(args.report).toEndWith(`opencode-acceptance-${process.platform}-${process.arch}.json`);
});

test.concurrent("--report and --only ids are read", () => {
  const args = parseAcceptanceArgs([
    "--report",
    "/tmp/r.json",
    "--only",
    "entry.npm-root",
    "pretool.mcp",
  ]);
  expect(args).toEqual({ report: "/tmp/r.json", only: ["entry.npm-root", "pretool.mcp"] });
});

test.concurrent("ids without --only, and --only without ids, are refused", () => {
  expect(() => parseAcceptanceArgs(["entry.npm-root"])).toThrow("use --only");
  expect(() => parseAcceptanceArgs(["--only"])).toThrow("--only needs at least one check id");
});

test.concurrent("an unknown check id exits 1 with its name before any host session", async () => {
  const res = await run([process.execPath, "run", "test:opencode", "--only", "nope.missing"], {
    cwd: ROOT,
    timeoutMs: 120_000,
  });
  expect(res.exitCode).toBe(1);
  expect(res.stderr).toContain("opencode-acceptance: unknown acceptance check: nope.missing");
});
