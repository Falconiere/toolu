import { expect, test } from "bun:test";
import { resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";

const ROOT = resolve(import.meta.dir, "../../..");
const SCRIPT = resolve(ROOT, "tooling/codex-smoke.sh");
const NO_CODEX = Bun.which("codex") === null;

test.concurrent.skipIf(NO_CODEX)(
  "Codex installs, lists, exercises, and removes every local plugin",
  async () => {
    const res = await run(["bash", SCRIPT], { cwd: ROOT, timeoutMs: 240_000 });
    const output = res.stdout + res.stderr;

    expect(res.timedOut).toBe(false);
    expect(res.exitCode).toBe(0);
    expect(output).toContain("available=16");
    expect(output).toContain("installed=16");
    expect(output).toContain("session-start=18");
    expect(output).toContain("removed=16");
  },
  300_000,
);
