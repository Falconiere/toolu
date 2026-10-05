/** Shared JSON cases and real sandbox runner for the protected-files, MCP, and edit-rule gates. */
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { toStdin } from "@toolu/conformance/harness/fixtures";
import { materializeToolFixture, readCaseFile } from "@toolu/conformance/harness/json-cases";
import { pretoolEnv, TOOLU_PLUGIN } from "@toolu/conformance/harness/pretool";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch } from "@toolu/conformance/harness/spawn";
import { z } from "zod";

export type Outcome = "deny" | "ask" | "advisory" | "silent";
export type Entry = "pre-tools" | "mcp-tools";

const ModuleCaseSchema = z.strictObject({
  name: z.string().min(1),
  host: z.enum(["claude", "codex"]),
  entry: z.enum(["pre-tools", "mcp-tools"]),
  fixture: z.unknown().optional(),
  stdin: z.string().optional(),
  settings: z.record(z.string(), z.string()).optional(),
  shippedSettings: z.boolean().optional(),
  config: z
    .strictObject({
      scope: z.enum(["project", "user"]),
      body: z.union([z.record(z.string(), z.unknown()), z.string()]),
    })
    .optional(),
  files: z.array(z.string()).optional(),
  expect: z.enum(["deny", "ask", "advisory", "silent"]),
  has: z.array(z.string()).optional(),
  lacks: z.array(z.string()).optional(),
  deviation: z.string().optional(),
});

export type ModuleCase = z.infer<typeof ModuleCaseSchema>;
export type Captured = { stdout: string; stderr: string; exitCode: number };

export const MODULE_CASES: readonly ModuleCase[] = z
  .array(ModuleCaseSchema)
  .parse(
    readCaseFile(resolve(import.meta.dir, "../../../../../fixtures/gates/pre-tool-modules-a.json")),
  );

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
      : JSON.stringify(
          toStdin(c.host, materializeToolFixture(sb, c.fixture, c.host), {
            cwd: sb.project,
          }),
        ));
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
