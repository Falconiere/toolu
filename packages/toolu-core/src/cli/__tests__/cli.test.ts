import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { CliExit, flagValue, numberValue } from "../cli.ts";

const CLI = join(import.meta.dir, "../cli.ts");

function thrown(run: () => unknown): CliExit {
  try {
    run();
  } catch (error) {
    if (error instanceof CliExit) return error;
    throw error;
  }
  throw new Error("expected a CliExit");
}

test("flagValue returns the argument after the flag", () => {
  expect(flagValue("tool", ["-q", "needle"], 0)).toBe("needle");
});

test("flagValue exits 1 naming the flag when its value is missing", () => {
  const exit = thrown(() => flagValue("tool", ["x", "-q"], 1));
  expect(exit.code).toBe(1);
  expect(exit.message).toBe("tool: -q needs a value");
});

test("numberValue accepts what jq --argjson accepts", () => {
  const accepted: Array<[string, number]> = [
    ["5", 5],
    ["2.5", 2.5],
    ["+5", 5],
    ["007", 7],
    ["5.", 5],
    [" 5", 5],
    ["1e2", 100],
    ["-3", -3],
  ];
  for (const [text, value] of accepted) expect(numberValue("tool", "-n", text)).toBe(value);
});

test("numberValue exits 2, as jq --argjson did, on what jq rejects", () => {
  for (const text of ["abc", "", " ", "5x", "0x10", "0b11", "Infinity"]) {
    const exit = thrown(() => numberValue("tool", "-n", text));
    expect(exit.code).toBe(2);
    expect(exit.message).toBe("tool: -n must be a number");
  }
});

/** Runs a tiny real CLI built on runCli in a subprocess. */
function runScript(body: string): { status: number | null; stdout: string; stderr: string } {
  const dir = mkdtempSync(join(tmpdir(), "toolu-cli-"));
  try {
    const script = join(dir, "main.ts");
    writeFileSync(script, `import { CliExit, runCli } from ${JSON.stringify(CLI)};\n${body}\n`);
    const run = spawnSync(process.execPath, [script], { encoding: "utf8" });
    return { status: run.status, stdout: run.stdout, stderr: run.stderr };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test("runCli exits with the returned status", () => {
  const run = runScript(
    `await runCli(async () => { await Bun.write(Bun.stdout, "ok\\n"); return 3; });`,
  );
  expect(run.status).toBe(3);
  expect(run.stdout).toBe("ok\n");
});

test("runCli writes a CliExit's stdout and message, then exits with its code", () => {
  const run = runScript(
    `await runCli(async () => { throw new CliExit(22, "tool: HTTP 401", "{}\\n"); });`,
  );
  expect(run.status).toBe(22);
  expect(run.stdout).toBe("{}\n");
  expect(run.stderr).toBe("tool: HTTP 401\n");
});

test("runCli maps an unexpected error to exit 1 with its message", () => {
  const run = runScript(`await runCli(async () => { throw new Error("boom"); });`);
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("boom");
});

test("a reader that goes away ends the CLI quietly with 141, as SIGPIPE did", () => {
  const dir = mkdtempSync(join(tmpdir(), "toolu-cli-"));
  try {
    const script = join(dir, "main.ts");
    writeFileSync(
      script,
      `import { runCli, writeStdout } from ${JSON.stringify(CLI)};\n` +
        `await runCli(async () => { await writeStdout("x".repeat(1 << 22)); return 0; });\n`,
    );
    const pipeline = `set -o pipefail; "$0" "$1" | head -c 10 >/dev/null`;
    const run = spawnSync("bash", ["-c", pipeline, process.execPath, script], { encoding: "utf8" });
    expect(run.status).toBe(141);
    expect(run.stderr).toBe("");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
