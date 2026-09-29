/**
 * PreToolUse corpus (#258), edit tools: protected-files, code-edit-rules and
 * the dispatcher's multi-file patch walk.
 */
import { bashFixture, editFixture, patchFixture, writeFixture } from "./fixtures.ts";
import { abs, mode, type PretoolCase } from "./pretool-case.ts";

export const EDIT_CASES: PretoolCase[] = [
  {
    name: "protected-files: .env edit asks",
    fixture: (sb) => editFixture(abs(".env")(sb), "a", "b"),
    expect: { claude: "ask", codex: "deny" },
  },
  {
    name: "protected-files: block mode denies",
    fixture: (sb) => editFixture(abs(".env")(sb), "a", "b"),
    config: mode("protectedFiles", "block"),
    expect: { claude: "deny" },
  },
  {
    name: "protected-files: advise mode advises",
    fixture: (sb) => writeFixture(abs(".env.local")(sb), "x"),
    config: mode("protectedFiles", "advise"),
    expect: { claude: "advisory" },
  },
  {
    name: "protected-files: shell write to .env asks",
    fixture: () => bashFixture("echo x > .env"),
    expect: { claude: "ask", codex: "deny" },
  },
  {
    name: "protected-files: off mode is silent",
    fixture: (sb) => editFixture(abs(".env")(sb), "a", "b"),
    config: mode("protectedFiles", "off"),
    expect: { claude: "silent" },
  },
  {
    name: "code-edit-rules: rust rules",
    fixture: (sb) => editFixture(abs("src/a.rs")(sb), "a", "b"),
    expect: { claude: "advisory" },
  },
  {
    name: "code-edit-rules: feature docs",
    fixture: (sb) => writeFixture(abs("src/features/x.ts")(sb), "export {};"),
    expect: { claude: "advisory" },
  },
  {
    name: "edit of a plain text file is silent",
    fixture: (sb) => writeFixture(abs("notes.txt")(sb), "hi"),
    expect: { claude: "silent" },
  },
  {
    name: "multi-file patch: protected second path",
    fixture: () =>
      patchFixture([
        { op: "update", path: "src/a.ts", lines: ["-a", "+b"] },
        { op: "update", path: ".env", lines: ["-a", "+b"] },
      ]),
    expect: { claude: "ask", codex: "deny" },
  },
  {
    name: "multi-file patch: blocked second path",
    fixture: () =>
      patchFixture([
        { op: "add", path: "src/b.rs", lines: ["x"] },
        { op: "delete", path: ".env" },
      ]),
    config: mode("protectedFiles", "block"),
    expect: { claude: "deny" },
  },
  {
    name: "multi-file patch: advisories only",
    fixture: () =>
      patchFixture([
        { op: "add", path: "src/b.rs", lines: ["x"] },
        { op: "add", path: "src/c.ts", lines: ["y"] },
      ]),
    expect: { claude: "advisory" },
  },
  {
    name: "malformed apply_patch fails closed",
    fixture: () => ({
      kind: "tool",
      event: "PreToolUse",
      toolName: "apply_patch",
      toolInput: { command: "garbage" },
    }),
    expect: { claude: "deny" },
  },
];
