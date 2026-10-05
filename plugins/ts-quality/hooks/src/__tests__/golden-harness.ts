/**
 * Runs one ts-quality golden case (#265) the way a host runs it: a real
 * sandbox repository, the ts-quality module registered into the host's
 * registry, and each tool call dispatched by the committed post-tools bundle
 * behind its hooks.json launcher. Only the registration differs between the
 * bash baseline (the pre-port `register.sh`, from git) and the TypeScript
 * module (`hooks/dist/register.js`), so the module is the one variable.
 */
import { spawnSync } from "node:child_process";
import { rmSync } from "node:fs";
import { join, resolve } from "node:path";
import { isJsonObject } from "@toolu/core/config";
import { launchedArgv } from "@toolu/conformance/harness/entry-command";
import {
  patchFixture,
  postToolFixture,
  toStdin,
  type Fixture,
  type ToolFixture,
} from "@toolu/conformance/harness/fixtures";
import { runPostBundle } from "@toolu/conformance/harness/posttool";
import {
  installPlugins,
  pretoolEnv,
  registerPlugin,
  type PretoolHost,
} from "@toolu/conformance/harness/pretool";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch } from "@toolu/conformance/harness/spawn";
import type { Step, TsCase } from "./cases-types.ts";

export const PLUGIN_ROOT = resolve(import.meta.dir, "../../..");
export const REPO_ROOT = resolve(PLUGIN_ROOT, "../..");

/** Which ts-quality module the sandbox registers. */
export type Registration =
  | { readonly kind: "bash"; readonly register: string }
  | { readonly kind: "bundle" };

export type StepResult = {
  stdout: string;
  stderr: string;
  exitCode: number;
  state: Record<string, string>;
};

/** Sandbox paths become `<ROOT>` so captures from different sandboxes compare. */
function normalise(text: string, root: string): string {
  return text.split(root).join("<ROOT>");
}

/**
 * The gate file with its top-level `violations` rebuilt in entry-key order.
 * Bash and TypeScript both order that aggregate by second-resolution
 * `updatedAt`, so entries written across a second boundary swap (#259). Left
 * as is unless the aggregate is exactly a reordering of the entries.
 */
export function canonicalGate(text: string): string {
  let doc: unknown;
  try {
    doc = JSON.parse(text);
  } catch {
    return text;
  }
  if (!isJsonObject(doc) || !isJsonObject(doc.entries) || typeof doc.violations !== "string")
    return text;
  const entries = doc.entries;
  const parts = Object.keys(entries)
    .toSorted()
    .map((key) => {
      const entry = entries[key];
      return isJsonObject(entry) && typeof entry.violations === "string" ? entry.violations : "";
    });
  const aggregate = doc.violations;
  const reordering =
    aggregate.length === parts.join("").length && parts.every((part) => aggregate.includes(part));
  return reordering ? `${JSON.stringify({ ...doc, violations: parts.join("") }, null, 2)}\n` : text;
}

async function registerTsQuality(sb: Sandbox, host: PretoolHost, reg: Registration) {
  const env = pretoolEnv(sb, host, { CLAUDE_PLUGIN_ROOT: PLUGIN_ROOT, PLUGIN_ROOT: undefined });
  if (host === "codex") env.PLUGIN_ROOT = PLUGIN_ROOT;
  const argv =
    reg.kind === "bash"
      ? ["bash", reg.register]
      : launchedArgv({ plugin: "ts-quality", event: "SessionStart", entry: "register" });
  const res = await run(argv, { cwd: sb.project, env, stdin: "{}" });
  if (res.exitCode !== 0 || res.stdout !== "") {
    throw new Error(`register ts-quality (${reg.kind}): ${String(res.exitCode)} ${res.stderr}`);
  }
}

function prepare(sb: Sandbox, host: PretoolHost, c: TsCase): void {
  // No detached gc/maintenance writing into `.git` while a hook runs.
  sb.git("config", "maintenance.auto", "false");
  sb.git("config", "gc.auto", "0");
  installPlugins(sb, "toolu@toolu", "ts-quality@toolu", "rust-quality@toolu");
  const committed = { ...c.project, ...c.commit };
  for (const [rel, body] of Object.entries(committed)) sb.write(rel, body);
  if (Object.keys(committed).length > 0) {
    sb.git("add", ...Object.keys(committed));
    sb.git("commit", "-q", "-m", "project");
  }
  if (c.config !== undefined) sb.writeConfig(host, "project", c.config);
  c.setup?.(sb);
}

/** A PostToolUse call of `toolName` that already ran. */
function ran(toolName: string, toolInput: Record<string, unknown>, response: unknown): ToolFixture {
  return { kind: "tool", event: "PostToolUse", toolName, toolInput, toolResponse: response };
}

function fixtureFor(sb: Sandbox, host: PretoolHost, step: Step): Fixture {
  if (step.patch !== undefined) return postToolFixture(patchFixture([...step.patch]), "Done");
  if (step.rawPatch !== undefined) return ran("apply_patch", { command: step.rawPatch }, "Done");
  const file = step.file ?? "";
  const path = step.relative === true ? file : sb.path(file);
  const tool = step.tool ?? "Write";
  if (tool === "Delete" && host === "codex") {
    return postToolFixture(patchFixture([{ op: "delete", path: file }]), "Done");
  }
  if (tool === "Delete") return ran("Edit", { file_path: path }, { success: true });
  const content = step.write?.[file] ?? "";
  const input = tool === "Write" ? { file_path: path, content } : { file_path: path };
  return ran(tool, { ...input, ...step.input }, { success: true });
}

/** A Claude deletion reaches the module as an Edit with `TOOLU_EDIT_OPERATION=delete` (bats). */
function stepEnv(sb: Sandbox, host: PretoolHost, c: TsCase, step: Step): EnvPatch {
  const deletion = step.tool === "Delete" && host === "claude";
  const extra: EnvPatch = { ...c.env?.(sb), ...step.env };
  return pretoolEnv(sb, host, deletion ? { ...extra, TOOLU_EDIT_OPERATION: "delete" } : extra);
}

/** One step dispatched by the post-tools bundle: the raw result, with its duration. */
export async function dispatchStep(sb: Sandbox, host: PretoolHost, c: TsCase, step: Step) {
  for (const [rel, body] of Object.entries(step.write ?? {})) sb.write(rel, body);
  for (const rel of step.remove ?? []) rmSync(sb.path(rel), { force: true });
  const stdin = JSON.stringify(toStdin(host, fixtureFor(sb, host, step), { cwd: sb.project }));
  return runPostBundle(sb, { cwd: sb.project, env: stepEnv(sb, host, c, step), stdin });
}

async function runStep(sb: Sandbox, host: PretoolHost, c: TsCase, step: Step): Promise<StepResult> {
  const res = await dispatchStep(sb, host, c, step);
  const state = Object.fromEntries(
    Object.entries(res.state).map(([k, v]) => [
      normalise(k, sb.root),
      normalise(k.endsWith("quality-gate-status.json") ? canonicalGate(v) : v, sb.root),
    ]),
  );
  return {
    stdout: normalise(res.stdout, sb.root),
    stderr: normalise(res.stderr, sb.root),
    exitCode: res.exitCode,
    state,
  };
}

/** Put `sb` in `c`'s starting state on `host`, with the given ts-quality module registered. */
export async function setupCase(sb: Sandbox, host: PretoolHost, c: TsCase, reg: Registration) {
  prepare(sb, host, c);
  await registerTsQuality(sb, host, reg);
  for (const plugin of c.register ?? []) await registerPlugin(sb, host, plugin);
}

/** Every step's result, in order, for `c` on `host` with the given module registered. */
export async function runCase(
  c: TsCase,
  host: PretoolHost,
  reg: Registration,
): Promise<StepResult[]> {
  using sb = createSandbox({ git: true });
  await setupCase(sb, host, c, reg);
  const results: StepResult[] = [];
  for (const step of c.steps) results.push(await runStep(sb, host, c, step));
  return results;
}

/**
 * The bash module as it was at `base`: `plugins/ts-quality/hooks` extracted
 * from git into `dir`, so it runs after the files are deleted. Returns its
 * `register.sh`.
 */
export function extractBaseRegister(base: string, dir: string): string {
  const tar = spawnSync("git", ["-C", REPO_ROOT, "archive", base, "plugins/ts-quality/hooks"]);
  if (tar.status !== 0) throw new Error(`git archive ${base} failed`);
  const untar = spawnSync("tar", ["-x", "-C", dir], { input: tar.stdout });
  if (untar.status !== 0) throw new Error("tar -x failed");
  return join(dir, "plugins/ts-quality/hooks/register.sh");
}

/** The commit the golden was captured at: the last with the bash module. */
export const BASH_BASE = "a8b0c9c9";

/** The golden key of `c` on `host`. */
export function caseKey(c: TsCase, host: PretoolHost): string {
  return `${c.name} [${host}]`;
}

export const GOLDEN_PATH = join(import.meta.dir, "fixtures", "golden.json");
