/**
 * search-nudge golden cases (#268). The first six are search-nudge.bats; its two
 * `TOOLU_LIB_DIR` sourcing tests have no TypeScript counterpart (the module
 * imports `@toolu/core`), and its "no source-repo names" test is an assertion
 * over every capture in `golden-nudge.test.ts`. The rest cover each ast-grep
 * state, every non-code allow path of the Grep branch, and the parsed-command
 * rules of the Bash branch, including #283 item 10.
 */
import type { Deviation, NudgeCase } from "./cases-types.ts";

const grep = (toolInput: Record<string, unknown>) => ({ toolName: "Grep", toolInput });
const bash = (command: string) => ({ toolName: "Bash", toolInput: { command } });

const BATS: NudgeCase[] = [
  { name: "bats: no-op on unrelated tool", toolName: "Read", toolInput: {} },
  {
    name: "bats: Grep with non-code glob allowed silently",
    ...grep({ pattern: "foo", glob: "*.md" }),
  },
  {
    name: "bats: Grep with structural pattern nudges to ast-grep",
    hosts: ["claude", "codex"],
    ...grep({ pattern: "fn handle_request", glob: "*.rs" }),
  },
  {
    name: "bats: Bash grep on structural pattern nudges to ast-grep",
    hosts: ["claude", "codex"],
    ...bash('grep -r "impl Foo" src'),
  },
  { name: "bats: Bash without grep is silent", ...bash("ls -la") },
  { name: "bats: output does not leak source-repo names", ...grep({ pattern: "fn x" }) },
];

const GREP_TOOL: NudgeCase[] = [
  { name: "Grep structural, ast-grep missing", state: "missing", ...grep({ pattern: "fn x" }) },
  { name: "Grep structural, ast-grep opted out", state: "opt-out", ...grep({ pattern: "fn x" }) },
  { name: "Grep plain identifier is silent", ...grep({ pattern: "handleRequest", glob: "*.ts" }) },
  { name: "Grep non-code type is silent", ...grep({ pattern: "fn x", type: "toml" }) },
  { name: "Grep code type still nudges", ...grep({ pattern: "impl Foo", type: "rust" }) },
  { name: "Grep non-code path is silent", ...grep({ pattern: "fn x", path: "docs/guide" }) },
  { name: "Grep code glob still nudges", ...grep({ pattern: "class Foo", glob: "*.{ts,tsx}" }) },
  { name: "Grep non-code glob is case-insensitive", ...grep({ pattern: "fn x", glob: "*.MD" }) },
  { name: "Grep multi-line pattern", ...grep({ pattern: "foo\nimpl Bar" }) },
  { name: "Grep arrow pattern", ...grep({ pattern: "(a) => b" }) },
];

const BASH_TOOL: NudgeCase[] = [
  { name: "Bash generic grep", hosts: ["claude", "codex"], ...bash("grep -rn TODO src") },
  { name: "Bash generic grep, ast-grep missing", state: "missing", ...bash("grep -rn TODO src") },
  { name: "Bash generic grep, ast-grep opted out", state: "opt-out", ...bash("grep -rn TODO src") },
  { name: "Bash structural grep", ...bash('grep -rn "pub fn main" src') },
  {
    name: "Bash structural grep, ast-grep missing",
    state: "missing",
    ...bash('grep -rn "pub fn main" src'),
  },
  {
    name: "Bash structural grep, ast-grep opted out",
    state: "opt-out",
    ...bash('grep -rn "pub fn main" src'),
  },
  { name: "Bash rg without a path", ...bash("rg TODO") },
  { name: "Bash grep piped onward", ...bash("grep -rn TODO src | head -5") },
  { name: "Bash grep under sudo", ...bash("sudo grep -r TODO /etc") },
  { name: "Bash xargs grep searches files", ...bash("git ls-files | xargs grep -n TODO") },
  { name: "Bash git grep", ...bash("git grep -n TODO") },
  { name: "Bash structural grep in a substitution", ...bash('echo $(grep -rn "pub fn x" src)') },
  { name: "Shell tool grep", toolName: "Shell", toolInput: { command: "grep -rn TODO src" } },
  { name: "283-10a: grep filtering a pipe", ...bash("git log | grep fix") },
  {
    name: "283-10b: grep and rg inside a commit message",
    ...bash('git commit -m "use grep and rg to find it"'),
  },
  {
    name: "283-10c: a shell for-in loop",
    ...bash('for f in src/*.ts; do grep -n TODO "$f"; done'),
  },
  { name: "283-10d: structural pattern as one argument", ...bash('rg -n "fn main" src/') },
  { name: "structural grep filtering a pipe", ...bash('cat src/a.rs | grep "pub fn main"') },
  { name: "grep as a word to echo", ...bash("echo grep me") },
  { name: "grep in a heredoc run by bash", ...bash("bash <<'EOF'\ngrep -rn TODO src\nEOF") },
];

export const NUDGE_CASES: readonly NudgeCase[] = [...BATS, ...GREP_TOOL, ...BASH_TOOL];

const STOP = "STOP: grep/rg for structural code search.";
const GENERIC = "grep/rg in Bash detected.";

/** Cases where bash was wrong; the TypeScript module must give this instead of the capture. */
export const NUDGE_DEVIATIONS: Readonly<Record<string, Deviation>> = {
  "bats: Bash grep on structural pattern nudges to ast-grep": { contains: STOP },
  "283-10a: grep filtering a pipe": { silent: true },
  "283-10b: grep and rg inside a commit message": {
    contains: "BEFORE COMMITTING",
    excludes: GENERIC,
  },
  "283-10c: a shell for-in loop": { contains: GENERIC, excludes: "STOP" },
  "283-10d: structural pattern as one argument": { contains: STOP },
  "structural grep filtering a pipe": { silent: true },
  "Bash structural grep in a substitution": { contains: STOP },
  "grep as a word to echo": { silent: true },
  "grep in a heredoc run by bash": { contains: GENERIC },
};
