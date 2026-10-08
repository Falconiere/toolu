/** An unreadable edited file leaves an explicit failing gate entry. */
import { chmodSync } from "node:fs";
import { expect, test } from "bun:test";
import { TS_PROJECT, type TsCase } from "./cases-types.ts";
import { runCase } from "./golden-harness.ts";

test("an unreadable TypeScript file fails the quality gate", async () => {
  // Root ignores the mode bit, so the file is not unreadable in this process.
  if (process.getuid?.() === 0) return;
  const caseFile: TsCase = {
    name: "unreadable TypeScript file",
    project: TS_PROJECT,
    setup: (sb) => {
      sb.write("src/unreadable.ts", 'console.log("unreadable");\n');
      chmodSync(sb.path("src/unreadable.ts"), 0);
    },
    steps: [{ tool: "Write", file: "src/unreadable.ts", relative: true }],
    expect: "advisory",
  };
  const [result] = await runCase(caseFile, "claude", { kind: "bundle" });
  expect(result?.stdout).toContain("Cannot read src/unreadable.ts for TypeScript quality checks");
  expect(result?.stdout).toContain("QUALITY VIOLATION");
  expect(JSON.parse(result?.state[".claude/tmp/quality-gate-status.json"] ?? "null")).toMatchObject(
    {
      status: "failing",
      entries: { "src/unreadable.ts": { source: "ts-quality-hook" } },
    },
  );
}, 60_000);
