/** SessionStart runs `toolu epic engine --ensure` against a native binary on PATH. */
import { expect, test } from "bun:test";
import { chmodSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { runStartupHook } from "@toolu/conformance/harness/startup";

const PLUGIN = resolve(import.meta.dir, "../../..");

function tooluBin(sb: Sandbox, body: string): string {
  const bin = join(sb.root, "bin");
  mkdirSync(bin, { recursive: true });
  writeFileSync(join(bin, "toolu"), body);
  chmodSync(join(bin, "toolu"), 0o755);
  return bin;
}

const NATIVE = `#!/bin/sh
if [ "$1" = "--hook-protocol" ]; then
  printf '1\\n'
  exit 0
fi
printf '%s\\n' "$*" >> "$TOOLU_RECORD"
exit "\${TOOLU_EXIT:-0}"
`;

test.concurrent("ensure runs and stays silent when the binary exits 0", async () => {
  using sb = createSandbox();
  const record = join(sb.root, "record");
  const res = await runStartupHook(PLUGIN, "engine-ensure", sb, {
    HOME: sb.home,
    PATH: `${tooluBin(sb, NATIVE)}:${process.env["PATH"] ?? ""}`,
    CLAUDE_PLUGIN_ROOT: PLUGIN,
    TOOLU_RECORD: record,
    TOOLU_BIN: undefined,
  });
  expect(res).toMatchObject({ exitCode: 0, stdout: "", stderr: "" });
  expect(readFileSync(record, "utf8")).toBe("epic engine --ensure\n");
});

test.concurrent("a failing ensure is a session message and exit 0", async () => {
  using sb = createSandbox();
  const res = await runStartupHook(PLUGIN, "engine-ensure", sb, {
    HOME: sb.home,
    PATH: `${tooluBin(sb, NATIVE)}:${process.env["PATH"] ?? ""}`,
    CLAUDE_PLUGIN_ROOT: PLUGIN,
    TOOLU_EXIT: "1",
    TOOLU_BIN: undefined,
  });
  expect(res.exitCode).toBe(0);
  expect(res.stdout).toContain("epic engine:");
});

test.concurrent("stays silent when no native toolu is on PATH", async () => {
  using sb = createSandbox();
  const res = await runStartupHook(PLUGIN, "engine-ensure", sb, {
    HOME: sb.home,
    PATH: "/root/.bun/bin:/usr/bin:/bin",
    CLAUDE_PLUGIN_ROOT: PLUGIN,
    TOOLU_BIN: undefined,
  });
  expect(res).toMatchObject({ exitCode: 0, stdout: "", stderr: "" });
});
