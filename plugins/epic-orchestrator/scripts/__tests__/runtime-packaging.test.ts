import { expect, test } from "bun:test";
import { cpSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

test("installed epic scripts use the bundled runtime without node_modules", async () => {
  const root = mkdtempSync(join(tmpdir(), "toolu-epic-installed-"));
  const plugin = join(root, "epic-orchestrator");
  try {
    cpSync(join(import.meta.dir, "../../"), plugin, { recursive: true });
    const runner = join(plugin, "runtime-check.ts");
    writeFileSync(
      runner,
      [
        'import { originOf } from "./scripts/checkouts.ts";',
        'import { mergeAttemptOutcome } from "./scripts/merge-gate.ts";',
        "console.log(JSON.stringify({",
        "  origin: originOf('/tmp'),",
        "  merge: mergeAttemptOutcome({ exitCode: 1, stdout: '', stderr: 'protected branch', timedOut: true, cancelled: false, truncated: false }),",
        "}));",
      ].join("\n"),
    );
    const proc = Bun.spawn([process.execPath, runner], {
      cwd: plugin,
      env: { PATH: process.env.PATH ?? "" },
      stdout: "pipe",
      stderr: "pipe",
    });
    const [stdout, stderr, exitCode] = await Promise.all([
      new Response(proc.stdout).text(),
      new Response(proc.stderr).text(),
      proc.exited,
    ]);
    expect({ exitCode, stderr, value: JSON.parse(stdout) }).toEqual({
      exitCode: 0,
      stderr: "",
      value: { origin: null, merge: "uncertain" },
    });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
