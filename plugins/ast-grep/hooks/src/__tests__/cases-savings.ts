/**
 * byte-savings golden cases (#268). The first five are byte-savings.bats. The
 * rest pin the jq measure (response shapes and field precedence, trailing
 * newlines, UTF-8), the Read full-size rules, session-id sanitising, the
 * ledger's config-root precedence, and the parsed `ast-grep`/`sg` detection.
 */
import { mkdirSync, symlinkSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import type { Deviation, SavingsCase } from "./cases-types.ts";

const BIG = "x".repeat(4000);

function big(sb: Sandbox): string {
  return sb.write("src/big.txt", BIG);
}

const read = (sid: string, file: (sb: Sandbox) => string, response: unknown) => (sb: Sandbox) => ({
  session_id: sid,
  tool_name: "Read",
  tool_input: { file_path: file(sb) },
  tool_response: response,
});

const shell =
  (sid: string, command: string, response: unknown, toolName = "Bash") =>
  () => ({
    session_id: sid,
    tool_name: toolName,
    tool_input: { command },
    tool_response: response,
  });

const tool = (sid: string, toolName: string, response: unknown) => () => ({
  session_id: sid,
  tool_name: toolName,
  tool_input: {},
  tool_response: response,
});

const BATS: SavingsCase[] = [
  {
    name: "bats: single-file Read records returned and full bytes",
    payload: read("s1", big, "hello"),
  },
  {
    name: "bats: ast-grep Bash result bytes, full 0",
    hosts: ["claude", "codex"],
    payload: shell("s2", "ast-grep run --pattern foo .", { stdout: "match line" }),
  },
  { name: "bats: unrelated tools write no ledger", payload: tool("s3", "Edit", "x") },
  {
    name: "bats: a plain Bash command is ignored",
    payload: shell("s4", "ls -la", { stdout: "files" }),
  },
  {
    name: "bats: Read of a missing file records full 0",
    payload: read("s5", () => "/no/such/file", "abc"),
  },
];

const MEASURE: SavingsCase[] = [
  { name: "Grep string response", payload: tool("m1", "Grep", "a.ts:1:x\nb.ts:2:y") },
  {
    name: "Grep object response is compact JSON",
    payload: tool("m2", "Grep", { mode: "files", filenames: ["a", "b"], numFiles: 2 }),
  },
  {
    name: "Glob array response is compact JSON",
    payload: tool("m3", "Glob", ["src/a.ts", "src/b.ts"]),
  },
  {
    name: "content wins over stdout",
    payload: shell("m4", "sg run -p x", { content: "c", stdout: "longer" }),
  },
  {
    name: "empty stdout records nothing",
    payload: shell("m5", "sg run -p x", { content: null, stdout: "" }),
  },
  {
    name: "false stdout falls through to output",
    payload: shell("m6", "sg run -p x", { stdout: false, output: "out" }),
  },
  {
    name: "array content prints as pretty JSON",
    payload: shell("m7", "sg run -p x", { content: ["a", "b"] }),
  },
  {
    name: "object content prints as pretty JSON",
    payload: shell("m8", "sg run -p x", { content: { k: 1, l: [true] } }),
  },
  { name: "number content", payload: shell("m9", "sg run -p x", { content: 42 }) },
  {
    name: "object without known fields is compact JSON",
    payload: shell("m10", "sg run -p x", { exit: 0, text: "é" }),
  },
  { name: "trailing newlines are not counted", payload: tool("m11", "Grep", "abc\n\n\n") },
  { name: "UTF-8 bytes are counted", payload: tool("m12", "Grep", "héllo ✓") },
  {
    name: "missing response counts as null",
    payload: () => ({ session_id: "m13", tool_name: "Grep", tool_input: {} }),
  },
  { name: "number response", payload: tool("m14", "Grep", 5) },
  { name: "empty string response records nothing", payload: tool("m15", "Grep", "") },
];

const READ: SavingsCase[] = [
  {
    name: "relative Read path resolves against cwd",
    payload: read(
      "r1",
      (sb) => {
        big(sb);
        return "src/big.txt";
      },
      "hi",
    ),
  },
  {
    name: "Read of a directory records full 0",
    payload: read(
      "r2",
      (sb) => {
        big(sb);
        return sb.path("src");
      },
      "hi",
    ),
  },
  {
    name: "Read through a symlink counts the target",
    payload: read(
      "r3",
      (sb) => {
        const target = big(sb);
        symlinkSync(target, sb.path("link.txt"));
        return sb.path("link.txt");
      },
      "hi",
    ),
  },
];

const SESSION: SavingsCase[] = [
  {
    name: "session id keeps only letters, digits and dashes",
    payload: tool("a/b c.d-1", "Grep", "x"),
  },
  { name: "session id with nothing left is unknown", payload: tool("../..", "Grep", "x") },
  {
    name: "missing session id is unknown",
    payload: () => ({ tool_name: "Grep", tool_input: {}, tool_response: "x" }),
  },
  {
    name: "numeric session id",
    payload: () => ({ session_id: 42, tool_name: "Grep", tool_input: {}, tool_response: "x" }),
  },
];

const ROOTS: SavingsCase[] = [
  {
    name: "TOOLU_CONFIG_DIR holds the ledger",
    env: (sb) => ({ TOOLU_CONFIG_DIR: sb.path("cfg") }),
    payload: tool("c1", "Grep", "x"),
  },
  {
    name: "an unwritable ledger dir records nothing",
    setup: (sb) => {
      const dir = join(sb.configDir("claude", "user"), "toolu");
      mkdirSync(dir, { recursive: true });
      writeFileSync(join(dir, "byte-savings"), "not a directory\n");
    },
    payload: tool("c2", "Grep", "x"),
  },
];

const DETECT: SavingsCase[] = [
  { name: "Shell tool running sg", payload: shell("d1", "sg run -p x .", "hit", "Shell") },
  { name: "ast-grep after cd", payload: shell("d2", "cd src && ast-grep run -p x", "hit") },
  { name: "sg under timeout", payload: shell("d3", "timeout 60 sg run -p x .", "hit") },
  { name: "ast-grep run by npx", payload: shell("d5", "npx ast-grep run -p x .", "hit") },
  { name: "ast-grep run by pnpm exec", payload: shell("d6", "pnpm exec ast-grep run -p x", "hit") },
  {
    name: "ast-grep as a word to echo",
    payload: shell("d4", 'echo "run ast-grep later"', "run ast-grep later"),
  },
];

export const SAVINGS_CASES: readonly SavingsCase[] = [
  ...BATS,
  ...MEASURE,
  ...READ,
  ...SESSION,
  ...ROOTS,
  ...DETECT,
];

/** Cases where bash's text match was wrong (#283 item 10): the ledger the TypeScript module leaves instead. */
export const SAVINGS_DEVIATIONS: Readonly<Record<string, Deviation>> = {
  "sg under timeout": { contains: '{"kind":"ast-grep","returned":3,"full":0}' },
  "ast-grep as a word to echo": { silent: true },
};
