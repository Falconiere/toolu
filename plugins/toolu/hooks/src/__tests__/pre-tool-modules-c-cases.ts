/** Shared JSON cases and real sandbox runner for workflow gates and agent-tier. */
import {
  mkdirSync,
  readdirSync,
  readFileSync,
  statSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { bashFixture, toStdin } from "@toolu/conformance/harness/fixtures";
import {
  ActionSchema,
  applyCaseSetup,
  materializeCaseValue,
  materializeToolFixture,
  readCaseFile,
  resolveFixturePath,
} from "@toolu/conformance/harness/json-cases";
import { pretoolEnv, TOOLU_PLUGIN } from "@toolu/conformance/harness/pretool";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { launchedArgv } from "@toolu/conformance/harness/entry-command";
import { z } from "zod";

export type Hook = "pre-tools" | "agent-tier";

const GateCaseSchema = z.strictObject({
  name: z.string().min(1),
  host: z.enum(["claude", "codex"]),
  hook: z.enum(["pre-tools", "agent-tier"]),
  setup: z.array(ActionSchema).optional(),
  command: z.string().optional(),
  fixture: z.unknown().optional(),
  stdin: z.string().optional(),
  config: z.union([z.record(z.string(), z.unknown()), z.string()]).optional(),
  cwd: z.strictObject({ $path: z.string() }).optional(),
  env: z.record(z.string(), z.unknown()).optional(),
  without: z.array(z.string()).optional(),
  expect: z.enum(["deny", "ask", "advisory", "silent"]),
  has: z.array(z.string()).optional(),
  lacks: z.array(z.string()).optional(),
  deviation: z.string().optional(),
});

export type GateCase = z.infer<typeof GateCaseSchema>;
export type CaseInput = Omit<GateCase, "host" | "hook"> & {
  host?: GateCase["host"];
  hook?: Hook;
};

/** Fill the standard hook and host for the additional parser boundary case. */
export function group(defaults: Partial<GateCase>): (c: CaseInput) => GateCase {
  return (c) => ({ host: "claude", hook: "pre-tools", ...defaults, ...c });
}

export type Captured = {
  stdout: string;
  stderr: string;
  exitCode: number;
  files: Record<string, string | null>;
};

export const BASE = "main";
export const MODULE_CASES: readonly GateCase[] = z
  .array(GateCaseSchema)
  .parse(
    readCaseFile(resolve(import.meta.dir, "../../../../../fixtures/gates/pre-tool-modules-c.json")),
  );

/** Every tool the hooks and their libs reach for. */
const TOOLS = [
  "bash",
  "sh",
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
  "python3",
  "jq",
];

/** A PATH of symlinks to every tool in `TOOLS` except `without`. */
function pathWithout(root: string, without: readonly string[]): string {
  const bin = join(root, "bin-path");
  mkdirSync(bin, { recursive: true });
  for (const tool of TOOLS.filter((t) => !without.includes(t))) {
    const found = Bun.which(tool);
    if (found !== null) symlinkSync(found, join(bin, tool));
  }
  return bin;
}

/** Paths under the sandbox root that are not hook state. */
function ignored(rel: string): boolean {
  const parts = rel.split("/");
  if (parts.includes(".git")) return true;
  if (parts[0] === "settings" || parts[0]?.startsWith("bin-") === true) return true;
  // Bun and other tools cache under HOME; only the host config dirs are state.
  return parts[0] === "home" && parts[1] !== ".claude" && parts[1] !== ".codex";
}

/** Every file under the sandbox root that could be hook state, by relative path. */
function snapshot(root: string, dir: string = root, out = new Map<string, string>()) {
  for (const entry of readdirSync(dir)) {
    const abs = join(dir, entry);
    const rel = relative(root, abs);
    if (ignored(rel)) continue;
    const stat = statSync(abs, { throwIfNoEntry: false });
    if (stat?.isDirectory() === true) snapshot(root, abs, out);
    else if (stat?.isFile() === true) out.set(rel, readFileSync(abs, "utf8"));
  }
  return out;
}

const ISO = /\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ/g;

function touched(
  before: Map<string, string>,
  after: Map<string, string>,
  norm: (text: string) => string,
): Record<string, string | null> {
  const files: Record<string, string | null> = {};
  for (const [rel, body] of after) {
    if (before.get(rel) !== body) files[rel] = norm(body).replace(ISO, "<time>");
  }
  for (const rel of before.keys()) if (!after.has(rel)) files[rel] = null;
  return files;
}

/** How a side spawns `hook` for plugin root `root`. */
export type Argv = (hook: Hook, root: string) => string[];

/** The committed bundle behind its generated launcher. */
export const bundleArgv: Argv = (hook, root) =>
  launchedArgv({ plugin: "toolu", event: "PreToolUse", entry: hook }, root);

/** Run `c` in a fresh sandbox with the plugin at `root`, the sandbox root normalised to `$ROOT`. */
export async function runCase(
  c: GateCase,
  argv: Argv = bundleArgv,
  root: string = TOOLU_PLUGIN,
): Promise<Captured> {
  using sb = createSandbox({ git: true, branch: BASE });
  sb.git("config", "maintenance.auto", "false");
  sb.git("config", "gc.auto", "0");
  const settings = join(sb.root, "settings");
  mkdirSync(settings, { recursive: true });
  if (c.config !== undefined) {
    const body = typeof c.config === "string" ? c.config : JSON.stringify(c.config);
    const path = join(sb.configDir(c.host, "project"), "toolu.config.json");
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, body);
  }
  applyCaseSetup(sb, c.setup ?? [], c.host);
  const cwd = c.cwd === undefined ? sb.project : resolveFixturePath(sb, c.cwd.$path, c.host);
  const stdin =
    c.stdin ??
    JSON.stringify(
      toStdin(
        c.host,
        c.fixture === undefined
          ? bashFixture(c.command ?? "")
          : materializeToolFixture(sb, c.fixture, c.host),
        { cwd },
      ),
    );
  const path = c.without === undefined ? {} : { PATH: pathWithout(sb.root, c.without) };
  const caseEnv =
    c.env === undefined
      ? {}
      : z.record(z.string(), z.string()).parse(materializeCaseValue(sb, c.env, c.host));
  const env = pretoolEnv(sb, c.host, {
    TOOLU_SETTINGS_DIR: settings,
    CLAUDE_PLUGIN_ROOT: root,
    ...(c.host === "codex" ? { PLUGIN_ROOT: root } : {}),
    ...path,
    ...caseEnv,
  });
  const before = snapshot(sb.root);
  const result = await run(argv(c.hook, root), { cwd, env, stdin });
  const norm = (text: string) => text.split(sb.root).join("$ROOT");
  return {
    stdout: norm(result.stdout),
    stderr: norm(result.stderr),
    exitCode: result.exitCode,
    files: touched(before, snapshot(sb.root), norm),
  };
}

/** What must match: the decision JSON (not its whitespace), exit code, stderr, touched files. */
export function comparable(result: Captured): Omit<Captured, "stdout"> & { stdout: unknown } {
  const stdout: unknown = result.stdout.trim() === "" ? "" : JSON.parse(result.stdout);
  return { ...result, stdout };
}
