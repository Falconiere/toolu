/**
 * Cases for the native protected-files, mcp-blocker and code-edit-rules gates
 * (#260). Every `@test` of their deleted bats suites is here with the same
 * input and intent, plus boundary cases. `fixtures/pre-tool-modules-a-golden.json`
 * holds what bash printed for each case at the base commit, before the bash
 * modules were deleted; `pre-tool-modules-a.test.ts` replays the bundles.
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import {
  bashFixture,
  editFixture,
  mcpFixture,
  toStdin,
  writeFixture,
  type Fixture,
} from "@toolu/conformance/harness/fixtures";
import { pretoolEnv, TOOLU_PLUGIN, type PretoolHost } from "@toolu/conformance/harness/pretool";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch } from "@toolu/conformance/harness/spawn";
import { z } from "zod";

export type Outcome = "deny" | "ask" | "advisory" | "silent";
export type Entry = "pre-tools" | "mcp-tools";

export type ModuleCase = {
  name: string;
  host: PretoolHost;
  /** The hooks.json entry the call reaches: the dispatcher or the `mcp__` hook. */
  entry: Entry;
  fixture?: (sb: Sandbox) => Fixture;
  /** Raw stdin instead of the rendered fixture. */
  stdin?: string;
  /** Files written into the settings directory (`TOOLU_SETTINGS_DIR`). */
  settings?: Record<string, string>;
  /** Start from a copy of the shipped `plugins/toolu/settings`. */
  shippedSettings?: boolean;
  config?: { scope: "project" | "user"; body: object | string };
  /** Project files that exist before the call. */
  files?: string[];
  expect: Outcome;
  /** Substrings the decision text must contain, and must not. */
  has?: string[];
  lacks?: string[];
  /**
   * Why bash answered differently: a #283 defect it had, or a contract of the
   * TypeScript core. Its golden result is kept as the baseline that differs.
   */
  deviation?: string;
};

export type Captured = { stdout: string; stderr: string; exitCode: number };

const BATS_PROTECTED = ".env\n.env.*\n**/secrets/**\nhooks/lib/**\nhooks/**/*.sh\n";
const PROTECTED = { "protected-files.txt": BATS_PROTECTED };
const BLOCKLIST = { "mcp-blocklist.txt": "exampleblocked\n" };
const RUST_RULE = {
  "code-edit-rules.json": '{ "rules": [ { "match": "*.rs", "docs": ["rust.md"] } ] }\n',
};

const abs = (rel: string) => (sb: Sandbox) => sb.path(rel);
const gateMode = (gate: string, mode: string) => ({
  scope: "project" as const,
  body: { version: 1, gates: { [gate]: { mode } } },
});

function edit(path: string | ((sb: Sandbox) => string)): (sb: Sandbox) => Fixture {
  return (sb) => editFixture(typeof path === "string" ? path : path(sb), "a", "b");
}

function multiEdit(path: string | ((sb: Sandbox) => string)): (sb: Sandbox) => Fixture {
  return (sb) => ({
    kind: "tool",
    event: "PreToolUse",
    toolName: "MultiEdit",
    toolInput: {
      file_path: typeof path === "string" ? path : path(sb),
      edits: [{ old_string: "a", new_string: "b" }],
    },
  });
}

function tool(toolName: string, toolInput: Record<string, unknown>): () => Fixture {
  return () => ({ kind: "tool", event: "PreToolUse", toolName, toolInput });
}

const bash = (command: string) => () => bashFixture(command);
const mcp =
  (server: string, name = "search") =>
  () =>
    mcpFixture(server, name, {});

function pf(c: Omit<ModuleCase, "entry" | "host"> & { host?: PretoolHost }): ModuleCase {
  return { host: "claude", entry: "pre-tools", settings: PROTECTED, ...c };
}

function cer(c: Omit<ModuleCase, "entry" | "host"> & { host?: PretoolHost }): ModuleCase {
  return { host: "claude", entry: "pre-tools", ...c };
}

function mb(c: Omit<ModuleCase, "entry" | "host"> & { host?: PretoolHost }): ModuleCase {
  return { host: "claude", entry: "mcp-tools", settings: BLOCKLIST, ...c };
}

/** #253: `loadConfig` rejects an unsupported envelope and every gate blocks; bash read it as `{}`. */
const ENVELOPE = "#253 fail-closed config envelope";

const BANNER = "SECURITY GUARDRAIL — OVERRIDE REQUESTED";

const PROTECTED_FILES_CASES: ModuleCase[] = [
  pf({
    name: "protected-files: asks on .env, a secrets file, with the loud banner",
    fixture: edit(".env"),
    expect: "ask",
    has: [BANNER, ".env", "WHY THIS IS GUARDED", "This is a secrets file", "THIS ONE CALL"],
    lacks: ["example/template"],
  }),
  pf({
    name: "protected-files: asks on hooks/lib/detect.sh as enforcement code",
    fixture: edit("hooks/lib/detect.sh"),
    expect: "ask",
    has: ["enforcement code"],
    lacks: ["secrets file"],
  }),
  pf({
    name: "protected-files: asks on MultiEdit of hooks/lib/detect.sh",
    fixture: multiEdit("hooks/lib/detect.sh"),
    expect: "ask",
  }),
  pf({ name: "protected-files: allows src/foo.ts", fixture: edit("src/foo.ts"), expect: "silent" }),
  pf({
    name: "protected-files: asks on an absolute path under hooks/lib/**",
    fixture: edit(abs("hooks/lib/detect.sh")),
    files: ["hooks/lib/detect.sh"],
    expect: "ask",
  }),
  pf({
    name: "protected-files: asks on an absolute path under hooks/**/*.sh",
    fixture: edit(abs("hooks/post-tools/modules/rust-quality.sh")),
    files: ["hooks/post-tools/modules/rust-quality.sh"],
    expect: "ask",
  }),
  pf({
    name: "protected-files: allows an absolute path outside the globs",
    fixture: edit(abs("src/foo.ts")),
    files: ["src/foo.ts"],
    expect: "silent",
  }),
  pf({
    name: "protected-files: .env.example is a template, not a secrets file",
    fixture: edit(".env.example"),
    expect: "ask",
    has: ["example/template env file"],
    lacks: ["This is a secrets file"],
  }),
  pf({
    name: "protected-files: mode block restores the hard deny",
    fixture: edit(".env"),
    config: gateMode("protectedFiles", "block"),
    expect: "deny",
    has: ["Blocked by gates.protectedFiles.mode='block'"],
  }),
  pf({
    name: "protected-files: mode off disables the check",
    fixture: edit(".env"),
    config: gateMode("protectedFiles", "off"),
    expect: "silent",
  }),
  pf({
    name: "protected-files: mode advise warns without stopping",
    fixture: edit(".env"),
    config: gateMode("protectedFiles", "advise"),
    expect: "advisory",
    has: ["NOT stopped"],
  }),
  pf({
    name: "protected-files: on Codex the guardrail blocks",
    host: "codex",
    fixture: edit(".env"),
    expect: "deny",
    has: [".env"],
  }),
  pf({
    name: "protected-files: an invalid config envelope fails closed",
    fixture: edit(".env"),
    config: { scope: "project", body: { version: 2 } },
    expect: "deny",
    deviation: ENVELOPE,
  }),
  pf({
    name: "protected-files: Bash redirect onto .env says WRITE and names it",
    fixture: bash("echo hi > .env"),
    expect: "ask",
    has: [BANNER, "would WRITE to .env", "secrets file"],
  }),
  pf({
    name: "protected-files: Bash sed -i on .env.example asks",
    fixture: bash("sed -i 's/a/b/' .env.example"),
    expect: "ask",
  }),
  pf({
    name: "protected-files: Bash python3 -c open(.env, w) asks",
    fixture: bash("python3 -c \"open('.env', 'w').write(x)\""),
    expect: "ask",
  }),
  pf({
    name: "protected-files: Bash tee onto .env asks",
    fixture: bash("echo hi | tee .env"),
    expect: "ask",
  }),
  pf({
    name: "protected-files: Bash write in an unquoted heredoc substitution asks",
    fixture: bash("cat <<EOF\n$(echo hi > .env)\nEOF\n"),
    expect: "ask",
  }),
  pf({
    name: "protected-files: Bash second open() names .env, not ok.txt",
    fixture: bash("python3 -c \"open('ok.txt','w'); open('.env','w')\""),
    expect: "ask",
    has: ["would WRITE to .env,"],
    lacks: ["would WRITE to ok.txt"],
  }),
  pf({
    name: "protected-files: Bash write to an unprotected path is allowed",
    fixture: bash("echo hi > /tmp/scratch.txt"),
    expect: "silent",
  }),
  pf({
    name: "protected-files: Bash read of .env.example is allowed",
    fixture: bash("cat .env.example"),
    expect: "silent",
  }),
  pf({
    name: "protected-files: the Shell tool is covered like Bash",
    fixture: tool("Shell", { command: "echo hi > .env" }),
    expect: "ask",
  }),
  pf({
    name: "protected-files: .git internals get their own reason",
    shippedSettings: true,
    settings: {},
    fixture: (sb) => writeFixture(sb.path(".git/config"), "x"),
    expect: "ask",
    has: ["git's internal state"],
  }),
  pf({
    name: "protected-files: a path under secrets/ reads as a secrets file",
    shippedSettings: true,
    settings: {},
    fixture: edit("config/secrets/key.txt"),
    expect: "ask",
    has: ["This is a secrets file"],
  }),
  pf({
    name: "protected-files: a lint config gets the generic reason",
    shippedSettings: true,
    settings: {},
    fixture: edit(".oxlintrc.json"),
    expect: "ask",
    has: ["listed in settings/protected-files.txt"],
  }),
  pf({
    name: "protected-files: an extglob pattern matches",
    settings: { "protected-files.txt": "@(foo|bar).cfg\n" },
    fixture: edit("deep/bar.cfg"),
    expect: "ask",
  }),
  pf({
    name: "protected-files: a non-string file_path is a malformed edit",
    fixture: tool("Edit", { file_path: 5, old_string: "a", new_string: "b" }),
    expect: "deny",
    has: ["Unable to parse apply_patch file headers"],
  }),
  pf({
    name: "protected-files: an empty Bash command is ignored",
    fixture: bash(""),
    expect: "silent",
  }),
  pf({
    name: "protected-files: no protected-files.txt means no check",
    settings: {},
    fixture: edit(".env"),
    expect: "silent",
  }),
  pf({
    name: "protected-files: a dynamic target is matched by its text",
    fixture: bash("echo x > $HOME/.env"),
    expect: "ask",
  }),
  pf({
    name: "protected-files: a quoted variable target is not a protected path",
    fixture: bash('echo x > "$OUT"'),
    expect: "silent",
  }),
  pf({
    name: "protected-files: the protected write in a chain is found",
    fixture: bash("cp a b && echo x >> .env.local"),
    expect: "ask",
  }),
  pf({
    name: "protected-files: 2>&1 is not a write target",
    fixture: bash("echo x 2>&1"),
    expect: "silent",
  }),
];

/** One Bash command per writer kind, and each #283 item 1–3 write fixture. */
const WRITE_COMMANDS: readonly { command: string; target: string; deviation?: string }[] = [
  { command: "echo SECRET=1 >.env", target: ".env", deviation: "#283 item 1" },
  { command: "echo SECRET=1>.env", target: ".env", deviation: "#283 item 1" },
  { command: "printf k=v 1>.env", target: ".env", deviation: "#283 item 1" },
  { command: "echo x &>.env", target: ".env", deviation: "#283 item 1" },
  { command: "echo x >| .env", target: ".env", deviation: "#283 item 1" },
  { command: "{ echo x; } >.env", target: ".env", deviation: "#283 item 1" },
  { command: "exec 3>.env", target: ".env", deviation: "#283 item 1" },
  { command: "echo x 2>.env", target: ".env", deviation: "#283 item 1" },
  { command: "cp src/.env apps/api/.env 2>&1", target: "apps/api/.env", deviation: "#283 item 2" },
  { command: "cp -t apps/api src/.env", target: "apps/api/.env", deviation: "#283 item 2" },
  { command: "mv .env.new .env 2>/dev/null", target: ".env", deviation: "#283 item 2" },
  { command: "bash -c 'echo x > .env'", target: ".env", deviation: "#283 item 3" },
  { command: 'sh -c "cp a .env"', target: ".env", deviation: "#283 item 3" },
  { command: "eval 'echo x > .env'", target: ".env", deviation: "#283 item 3" },
  { command: "echo x | tee -a .env", target: ".env" },
  { command: "sed -i 's/a/b/' .env.local", target: ".env.local" },
  { command: "perl -i -pe 's/a/b/' .env", target: ".env" },
  { command: "mv x .env", target: ".env" },
  { command: "install -m 644 x .env", target: ".env" },
  { command: "dd if=/dev/zero of=.env", target: ".env" },
  { command: "python3 -c \"open('.env','w')\"", target: ".env" },
  {
    command: "echo x > .en[v]",
    target: ".env",
    deviation: "#284: a pattern target is every path it matches",
  },
];

const WRITE_CASES: ModuleCase[] = WRITE_COMMANDS.flatMap(({ command, target, deviation }) =>
  (["claude", "codex"] as const).map((host) =>
    pf({
      name: `protected-files [${host}]: ${command}`,
      host,
      fixture: bash(command),
      files: [".env"],
      expect: host === "claude" ? "ask" : "deny",
      has: [`would WRITE to ${target}`],
      ...(deviation === undefined ? {} : { deviation }),
    }),
  ),
);

const RULES_SHIPPED = { shippedSettings: true, settings: {} };

const CODE_EDIT_RULES_CASES: ModuleCase[] = [
  cer({
    name: "code-edit-rules: empty rules are a no-op",
    settings: { "code-edit-rules.json": '{"rules":[]}\n' },
    fixture: edit("/abs/src/foo.rs"),
    expect: "silent",
  }),
  cer({
    name: "code-edit-rules: a missing file is a no-op",
    fixture: edit("/abs/src/foo.rs"),
    expect: "silent",
  }),
  cer({
    name: "code-edit-rules: a matching glob surfaces docs",
    settings: RUST_RULE,
    fixture: edit("/abs/src/foo.rs"),
    expect: "advisory",
    has: ["File: /abs/src/foo.rs\nApply these rules: rust.md"],
  }),
  cer({
    name: "code-edit-rules: MultiEdit on a .rs file surfaces docs",
    settings: {
      "code-edit-rules.json":
        '{ "rules": [ { "match": "*.rs", "docs": ["Rust: zero compiler/clippy warnings"] } ] }\n',
    },
    fixture: multiEdit("/abs/src/foo.rs"),
    expect: "advisory",
    has: ["Rust: zero compiler/clippy warnings"],
  }),
  cer({
    name: "code-edit-rules: an absolute path in the repo matches a repo-relative glob",
    settings: {
      "code-edit-rules.json": '{ "rules": [ { "match": "src/**/*.rs", "docs": ["rust.md"] } ] }\n',
    },
    fixture: edit(abs("src/foo/bar.rs")),
    files: ["src/foo/bar.rs"],
    expect: "advisory",
    has: ["File: src/foo/bar.rs\n"],
  }),
  cer({
    name: "code-edit-rules: shipped rules add feature docs",
    ...RULES_SHIPPED,
    fixture: (sb) => writeFixture(sb.path("src/features/x.ts"), "export {};"),
    expect: "advisory",
    has: ["frontend: colocate by feature"],
  }),
  cer({
    name: "code-edit-rules: shipped rules without a feature path",
    ...RULES_SHIPPED,
    fixture: edit("lib/x.ts"),
    expect: "advisory",
    lacks: ["frontend: colocate by feature"],
  }),
  cer({
    name: "code-edit-rules: the first matching rule wins even without docs",
    settings: {
      "code-edit-rules.json":
        '{"rules":[{"match":"*.rs","docs":[]},{"match":"*.rs","docs":["second"]}]}\n',
    },
    fixture: edit("a.rs"),
    expect: "silent",
  }),
  cer({
    name: "code-edit-rules: extra keys and missing docs do not disable other rules",
    settings: {
      "code-edit-rules.json":
        '{"rules":[{"match":"*.py","note":1},{"match":"*.rs","docs":["rs"],"x":true}]}\n',
    },
    fixture: edit("a.rs"),
    expect: "advisory",
    has: ["Apply these rules: rs"],
  }),
  cer({
    name: "code-edit-rules: malformed JSON is a no-op",
    settings: { "code-edit-rules.json": "{not json\n" },
    fixture: edit("a.rs"),
    expect: "silent",
  }),
  cer({
    name: "code-edit-rules: non-string docs are joined as JSON text",
    settings: { "code-edit-rules.json": '{"rules":[{"match":"*.rs","docs":["a",5,true]}]}\n' },
    fixture: edit("a.rs"),
    expect: "advisory",
    has: ["Apply these rules: a + 5 + true"],
  }),
  cer({
    name: "code-edit-rules: Bash is not an edit",
    settings: RUST_RULE,
    fixture: bash("touch a.rs"),
    expect: "silent",
  }),
];

const MCP_BLOCKER_CASES: ModuleCase[] = [
  mb({
    name: "mcp-blocker: asks on a listed server, pointing at the blocklist file",
    fixture: mcp("exampleblocked"),
    expect: "ask",
    has: ["mcp-blocklist.txt"],
    lacks: ["toolu config"],
  }),
  mb({
    name: "mcp-blocker: allows an unlisted server",
    fixture: mcp("other", "do_thing"),
    expect: "silent",
  }),
  mb({
    name: "mcp-blocker: a prefix entry blocks a longer server name",
    settings: { "mcp-blocklist.txt": "claude_ai_\n" },
    fixture: mcp("claude_ai_Canva"),
    expect: "ask",
  }),
  mb({
    name: "mcp-blocker: a prefix entry does not block a different prefix",
    settings: { "mcp-blocklist.txt": "claude_ai_\n" },
    fixture: mcp("friend_ai_Canva"),
    expect: "silent",
  }),
  mb({
    name: "mcp-blocker: an exact server name blocks",
    fixture: mcp("exampleblocked", "save"),
    expect: "ask",
  }),
  mb({
    name: "mcp-blocker: on Codex a listed server is blocked",
    host: "codex",
    fixture: mcp("exampleblocked"),
    expect: "deny",
    has: ["exampleblocked"],
  }),
  mb({
    name: "mcp-blocker: config mcp.<server>=false blocks without the file",
    settings: {},
    config: { scope: "user", body: { version: 1, mcp: { someserver: false } } },
    fixture: mcp("someserver", "do"),
    expect: "ask",
    has: ["someserver", "toolu config"],
  }),
  mb({
    name: "mcp-blocker: a non-object config mcp neither blocks nor crashes",
    settings: {},
    config: { scope: "user", body: { version: 1, mcp: "broken-not-an-object" } },
    fixture: mcp("someserver", "do"),
    expect: "silent",
  }),
  mb({
    name: "mcp-blocker: a redirect hint is appended to the reason",
    settings: { "mcp-blocklist.txt": "someserver -> use the jira skill instead\n" },
    fixture: mcp("someserver", "do"),
    expect: "ask",
    has: ["use the jira skill instead"],
  }),
  mb({
    name: "mcp-blocker: a commented entry does not block",
    settings: { "mcp-blocklist.txt": "# claude_ai_Atlassian -> use the jira skill instead\n" },
    fixture: mcp("claude_ai_Atlassian", "createIssue"),
    expect: "silent",
  }),
  mb({ name: "mcp-blocker: empty stdin", stdin: "", expect: "silent" }),
  mb({ name: "mcp-blocker: non-JSON stdin", stdin: "not json\n", expect: "silent" }),
  mb({
    name: "mcp-blocker: a tool name without a server segment",
    fixture: tool("mcp__exampleblocked", {}),
    expect: "silent",
  }),
  mb({
    name: "mcp-blocker: mode block denies",
    config: gateMode("mcpBlocker", "block"),
    fixture: mcp("exampleblocked"),
    expect: "deny",
  }),
  mb({
    name: "mcp-blocker: mode advise warns without stopping",
    config: gateMode("mcpBlocker", "advise"),
    fixture: mcp("exampleblocked"),
    expect: "advisory",
    has: ["NOT stopped"],
  }),
  mb({
    name: "mcp-blocker: mode off is silent",
    config: gateMode("mcpBlocker", "off"),
    fixture: mcp("exampleblocked"),
    expect: "silent",
  }),
  mb({
    name: "mcp-blocker: an invalid config envelope still blocks a listed server",
    config: { scope: "project", body: { version: 2 } },
    fixture: mcp("exampleblocked"),
    expect: "deny",
    deviation: ENVELOPE,
  }),
  mb({
    name: "mcp-blocker: a whitespace-padded entry still matches",
    settings: { "mcp-blocklist.txt": "  exampleblocked  \n" },
    fixture: mcp("exampleblocked"),
    expect: "ask",
  }),
  mb({
    name: "mcp-blocker: config mcp value 'false' as a string does not block",
    settings: {},
    config: { scope: "user", body: { version: 1, mcp: { someserver: "false" } } },
    fixture: mcp("someserver", "do"),
    expect: "silent",
  }),
  mb({
    name: "mcp-blocker: no blocklist and no config is silent",
    settings: {},
    fixture: mcp("exampleblocked"),
    expect: "silent",
  }),
];

export const MODULE_CASES: readonly ModuleCase[] = [
  ...PROTECTED_FILES_CASES,
  ...WRITE_CASES,
  ...CODE_EDIT_RULES_CASES,
  ...MCP_BLOCKER_CASES,
];

function writeAt(path: string, body: string): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, body);
}

const SHIPPED_FILES = ["protected-files.txt", "mcp-blocklist.txt", "code-edit-rules.json"];

/** Put a fresh sandbox in `c`'s state; returns the call's env and stdin. */
async function prepare(sb: Sandbox, c: ModuleCase): Promise<{ env: EnvPatch; stdin: string }> {
  const settings = join(sb.root, "settings");
  mkdirSync(settings, { recursive: true });
  if (c.shippedSettings === true) {
    for (const file of SHIPPED_FILES) {
      writeAt(join(settings, file), await Bun.file(join(TOOLU_PLUGIN, "settings", file)).text());
    }
  }
  for (const [file, body] of Object.entries(c.settings ?? {})) writeAt(join(settings, file), body);
  for (const file of c.files ?? []) writeAt(sb.path(file), "x\n");
  if (c.config !== undefined) {
    const dir = sb.configDir(c.host, c.config.scope);
    const body = c.config.body;
    writeAt(join(dir, "toolu.config.json"), typeof body === "string" ? body : JSON.stringify(body));
  }
  const stdin =
    c.stdin ??
    (c.fixture === undefined
      ? ""
      : JSON.stringify(toStdin(c.host, c.fixture(sb), { cwd: sb.project })));
  return { env: pretoolEnv(sb, c.host, { TOOLU_SETTINGS_DIR: settings }), stdin };
}

/** How a case's hook command is spawned: capture (bash) or replay (bundle). */
export type Runner = (entry: Entry) => string[];

/** Run `c` in a fresh sandbox through `runner`, with the sandbox root normalised to `$ROOT`. */
export async function runCase(c: ModuleCase, runner: Runner): Promise<Captured> {
  using sb = createSandbox({ git: true });
  const { env, stdin } = await prepare(sb, c);
  const result = await run(runner(c.entry), { cwd: sb.project, env, stdin });
  const norm = (text: string) => text.split(sb.root).join("$ROOT");
  return { stdout: norm(result.stdout), stderr: norm(result.stderr), exitCode: result.exitCode };
}

const HookOutputSchema = z.object({
  hookSpecificOutput: z
    .object({
      permissionDecision: z.string().optional(),
      permissionDecisionReason: z.string().optional(),
      additionalContext: z.string().optional(),
    })
    .optional(),
  systemMessage: z.string().optional(),
});

/** The decision class and its text (reason, else context) of a hook's stdout. */
export function decisionOf(stdout: string): { outcome: Outcome; text: string } {
  if (stdout.trim() === "") return { outcome: "silent", text: "" };
  const out = HookOutputSchema.parse(JSON.parse(stdout));
  const hso = out.hookSpecificOutput;
  const permission = hso?.permissionDecision;
  if (permission === "deny" || permission === "ask") {
    return { outcome: permission, text: hso?.permissionDecisionReason ?? "" };
  }
  return { outcome: "advisory", text: hso?.additionalContext ?? out.systemMessage ?? "" };
}
