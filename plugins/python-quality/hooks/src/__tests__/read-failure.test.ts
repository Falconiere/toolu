/** AC-6 (#266, DEV-2): an unreadable edited file leaves an explicit failing gate entry. */
import { chmodSync } from "node:fs";
import { expect, test } from "bun:test";
import { PY_PROJECT, type PyCase } from "./cases-types.ts";
import { runCase } from "./golden-harness.ts";

test("an unreadable Python file fails the quality gate", async () => {
  if (process.getuid?.() === 0) return;
  const caseFile: PyCase = {
    name: "unreadable Python file",
    project: PY_PROJECT,
    setup: (sb) => {
      sb.write("app/unreadable.py", "x = 1  # type: ignore\n");
      chmodSync(sb.path("app/unreadable.py"), 0);
    },
    steps: [{ tool: "Write", file: "app/unreadable.py", relative: true }],
    expect: "advisory",
  };
  const [result] = await runCase(caseFile, "claude", { kind: "bundle" });
  expect(result?.stdout).toContain("Cannot read app/unreadable.py for Python quality checks");
  expect(result?.stdout).toContain("QUALITY VIOLATION");
  expect(JSON.parse(result?.state[".claude/tmp/quality-gate-status.json"] ?? "null")).toMatchObject(
    {
      status: "failing",
      entries: { "app/unreadable.py": { source: "python-quality-hook" } },
    },
  );
}, 60_000);
