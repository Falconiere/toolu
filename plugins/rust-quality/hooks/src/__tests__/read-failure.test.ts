/** AC-8 (#267): an unreadable edited file leaves an explicit failing gate entry. */
import { chmodSync } from "node:fs";
import { expect, test } from "bun:test";
import { RUST_PROJECT, type RsCase } from "./cases-types.ts";
import { runCase } from "./golden-harness.ts";

test("an unreadable Rust file fails the quality gate", async () => {
  const caseFile: RsCase = {
    name: "unreadable Rust file",
    project: RUST_PROJECT,
    setup: (sb) => {
      sb.write("src/unreadable.rs", "fn helper() {}\n");
      chmodSync(sb.path("src/unreadable.rs"), 0);
    },
    steps: [{ tool: "Write", file: "src/unreadable.rs", relative: true }],
    expect: "advisory",
  };
  const [result] = await runCase(caseFile, "claude", { kind: "bundle" });
  expect(result?.stdout).toContain("Cannot read src/unreadable.rs for Rust quality checks");
  expect(result?.stdout).toContain("QUALITY VIOLATION");
  expect(JSON.parse(result?.state[".claude/tmp/quality-gate-status.json"] ?? "null")).toMatchObject(
    {
      status: "failing",
      entries: { "src/unreadable.rs": { source: "rust-quality-hook" } },
    },
  );
}, 60_000);
