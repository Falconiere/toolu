/** Shared JSON cases and real sandbox runner for bash, commit, and quality gates. */
import { mkdirSync, symlinkSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { bashFixture, toStdin } from "@toolu/conformance/harness/fixtures";
import {
  ActionSchema,
  applyCaseSetup,
  materializeToolFixture,
  readCaseFile,
  resolveFixturePath,
} from "@toolu/conformance/harness/json-cases";
import { pretoolEnv, TOOLU_PLUGIN, type PretoolHost } from "@toolu/conformance/harness/pretool";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch } from "@toolu/conformance/harness/spawn";
import { z } from "zod";

export type Outcome = "deny" | "ask" | "advisory" | "silent";

const ModuleCaseSchema = z.strictObject({
  name: z.string().min(1),
  host: z.enum(["claude", "codex"]),
  command: z.string().optional(),
  fixture: z.unknown().optional(),
  settings: z.record(z.string(), z.string()),
  shippedSettings: z.boolean().optional(),
  config: z.union([z.record(z.string(), z.unknown()), z.string()]).optional(),
  gate: z.string().optional(),
  setup: z.array(ActionSchema).optional(),
  cwd: z.strictObject({ $path: z.string() }).optional(),
  env: z.record(z.string(), z.string()).optional(),
  noPython: z.boolean().optional(),
  expect: z.enum(["deny", "ask", "advisory", "silent"]),
  has: z.array(z.string()).optional(),
  lacks: z.array(z.string()).optional(),
  deviation: z.string().optional(),
});

export type ModuleCase = z.infer<typeof ModuleCaseSchema>;
export type Captured = { stdout: string; stderr: string; exitCode: number };
export type CaseInput = Omit<ModuleCase, "host" | "settings"> & {
  host?: ModuleCase["host"];
  settings?: Record<string, string>;
};

/** Fill the defaults used by the extra parser-boundary tests. */
export function group(defaults: Partial<ModuleCase>): (c: CaseInput) => ModuleCase {
  return (c) => ({ host: "claude", settings: {}, ...defaults, ...c });
}

export function gateMode(gate: string, mode: string): Record<string, unknown> {
  return { version: 1, gates: { [gate]: { mode } } };
}

export const FAILING_GATE = '{"status":"failing","reason":"forced","violations":""}\n';

export const MODULE_CASES: readonly ModuleCase[] = z
  .array(ModuleCaseSchema)
  .parse(
    readCaseFile(resolve(import.meta.dir, "../../../../../fixtures/gates/pre-tool-modules-b.json")),
  );

function writeAt(path: string, body: string): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, body);
}

/** Every tool `mod.sh`, its libs and the launcher reach for, except `python3`. */
const NO_PYTHON_TOOLS = [
  "bash",
  "sh",
  "jq",
  "git",
  "awk",
  "grep",
  "sed",
  "tr",
  "cat",
  "head",
  "tail",
  "dirname",
  "basename",
  "mktemp",
  "rm",
  "mkdir",
  "env",
  "uname",
  "cut",
  "sort",
  "uniq",
  "wc",
  "date",
  "printf",
  "ls",
  "find",
  "xargs",
  "readlink",
  "realpath",
  "stat",
  "touch",
  "mv",
  "cp",
  "ln",
  "tee",
  "sleep",
  "id",
  "od",
  "shasum",
  "chmod",
  "cmp",
  "diff",
  "comm",
  "paste",
  "expr",
];

function pathWithoutPython(root: string): string {
  const bin = join(root, "bin-no-python");
  mkdirSync(bin, { recursive: true });
  for (const tool of NO_PYTHON_TOOLS) {
    const found = Bun.which(tool);
    if (found !== null) symlinkSync(found, join(bin, tool));
  }
  return bin;
}

function stateDir(host: PretoolHost): string {
  return host === "codex" ? ".codex" : ".claude";
}

/** Put a fresh sandbox in `c`'s state; returns the call's cwd, env and stdin. */
async function prepare(
  sb: Sandbox,
  c: ModuleCase,
): Promise<{ cwd: string; env: EnvPatch; stdin: string }> {
  sb.git("config", "maintenance.auto", "false");
  sb.git("config", "gc.auto", "0");
  const settings = join(sb.root, "settings");
  mkdirSync(settings, { recursive: true });
  if (c.shippedSettings === true) {
    for (const file of ["bash-allowlist.txt", "bash-denylist.txt", "commit-prefixes.txt"]) {
      writeAt(join(settings, file), await Bun.file(join(TOOLU_PLUGIN, "settings", file)).text());
    }
  }
  for (const [file, body] of Object.entries(c.settings)) writeAt(join(settings, file), body);
  if (c.config !== undefined) {
    const body = typeof c.config === "string" ? c.config : JSON.stringify(c.config);
    writeAt(join(sb.configDir(c.host, "project"), "toolu.config.json"), body);
  }
  if (c.gate !== undefined) {
    writeAt(sb.path(`${stateDir(c.host)}/tmp/quality-gate-status.json`), c.gate);
  }
  applyCaseSetup(sb, c.setup ?? [], c.host);
  const cwd = c.cwd === undefined ? sb.project : resolveFixturePath(sb, c.cwd.$path, c.host);
  const fixture =
    c.fixture === undefined
      ? bashFixture(c.command ?? "")
      : materializeToolFixture(sb, c.fixture, c.host);
  const stdin = JSON.stringify(toStdin(c.host, fixture, { cwd }));
  const path = c.noPython === true ? { PATH: pathWithoutPython(sb.root) } : {};
  const env = pretoolEnv(sb, c.host, { TOOLU_SETTINGS_DIR: settings, ...path, ...c.env });
  return { cwd, env, stdin };
}

/** The hook command a case is spawned through: capture (bash) or replay (bundle). */
export type Runner = () => string[];

/** Run `c` in a fresh sandbox through `runner`, with the sandbox root normalised to `$ROOT`. */
export async function runCase(c: ModuleCase, runner: Runner): Promise<Captured> {
  using sb = createSandbox({ git: true });
  const { cwd, env, stdin } = await prepare(sb, c);
  const result = await run(runner(), { cwd, env, stdin });
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

/** What must match: the decision JSON (not its whitespace), the exit code, stderr. */
export function comparable(result: Captured): {
  stdout: unknown;
  stderr: string;
  exitCode: number;
} {
  const stdout: unknown = result.stdout.trim() === "" ? "" : JSON.parse(result.stdout);
  return { stdout, stderr: result.stderr, exitCode: result.exitCode };
}
