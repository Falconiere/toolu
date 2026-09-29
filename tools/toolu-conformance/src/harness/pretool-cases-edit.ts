/**
 * PreToolUse corpus (#258), edit tools: protected-files, code-edit-rules and
 * the dispatcher's multi-file patch walk.
 */
import { bashFixture, editFixture, patchFixture, writeFixture } from "./fixtures.ts";
import { abs, mode, type PretoolCase } from "./pretool-case.ts";

export const EDIT_CASES: PretoolCase[] = [
  {
    name: "protected-files: .env edit asks",
    entry: "pre-tools",
    fixture: (sb) => editFixture(abs(".env")(sb), "a", "b"),
    expect: { claude: "ask", codex: "deny" },
  },
  {
    name: "protected-files: block mode denies",
    entry: "pre-tools",
    fixture: (sb) => editFixture(abs(".env")(sb), "a", "b"),
    config: mode("protectedFiles", "block"),
    expect: { claude: "deny" },
  },
  {
    name: "protected-files: advise mode advises",
    entry: "pre-tools",
    fixture: (sb) => writeFixture(abs(".env.local")(sb), "x"),
    config: mode("protectedFiles", "advise"),
    expect: { claude: "advisory" },
  },
  {
    name: "protected-files: shell write to .env asks",
    entry: "pre-tools",
    fixture: () => bashFixture("echo x > .env"),
    expect: { claude: "ask", codex: "deny" },
  },
  {
    name: "protected-files: off mode is silent",
    entry: "pre-tools",
    fixture: (sb) => editFixture(abs(".env")(sb), "a", "b"),
    config: mode("protectedFiles", "off"),
    expect: { claude: "silent" },
  },
  {
    name: "code-edit-rules: rust rules",
    entry: "pre-tools",
    fixture: (sb) => editFixture(abs("src/a.rs")(sb), "a", "b"),
    expect: { claude: "advisory" },
  },
  {
    name: "code-edit-rules: feature docs",
    entry: "pre-tools",
    fixture: (sb) => writeFixture(abs("src/features/x.ts")(sb), "export {};"),
    expect: { claude: "advisory" },
  },
  {
    name: "edit of a plain text file is silent",
    entry: "pre-tools",
    fixture: (sb) => writeFixture(abs("notes.txt")(sb), "hi"),
    expect: { claude: "silent" },
  },
  {
    name: "multi-file patch: protected second path",
    entry: "pre-tools",
    fixture: () =>
      patchFixture([
        { op: "update", path: "src/a.ts", lines: ["-a", "+b"] },
        { op: "update", path: ".env", lines: ["-a", "+b"] },
      ]),
    expect: { claude: "ask", codex: "deny" },
  },
  {
    name: "multi-file patch: blocked second path",
    entry: "pre-tools",
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
    entry: "pre-tools",
    fixture: () =>
      patchFixture([
        { op: "add", path: "src/b.rs", lines: ["x"] },
        { op: "add", path: "src/c.ts", lines: ["y"] },
      ]),
    expect: { claude: "advisory" },
  },
  {
    name: "malformed apply_patch fails closed",
    entry: "pre-tools",
    fixture: () => ({
      kind: "tool",
      event: "PreToolUse",
      toolName: "apply_patch",
      toolInput: { command: "garbage" },
    }),
    expect: { claude: "deny" },
  },
];
