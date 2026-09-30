/**
 * `@toolu/core/dispatch`: the PreToolUse (#258) and PostToolUse (#259)
 * dispatchers, TypeScript ports of `pre-tools/mod.sh`, `post-tools/mod.sh` and
 * `toolu_dispatch_hook`. An edit tool is walked once per affected path (a deny,
 * block or exit 2 on any path wins for the whole patch); everything else is
 * walked once. Built-in modules are native gates (#260–#262); registry `.sh`
 * modules still run on bash, and their raw hook text is merged byte for byte
 * as the bash dispatcher merged it.
 */
import { isJsonObject, loadConfig } from "../config/config-load.ts";
import { enabled } from "../config/config-read.ts";
import { detectHost } from "../host/host-detect.ts";
import type { HostEnv, HostName } from "../host/host-name.ts";
import { configRoot, gitToplevel, projectRoot } from "../host/host-roots.ts";
import { normalizeEditRecords } from "../state/edit-records.ts";
import type { EditRecord } from "../state/state-schema.ts";
import { toJqJson } from "../state/state-io.ts";
import { childEnv, type ModuleResult } from "./dispatch-bash.ts";
import type { HookPhase, Session, ToolModule } from "./dispatch-context.ts";
import { parseDocument, readField, substituted } from "./dispatch-output.ts";
import { consume, dispatchModules, newWalkState, settle, type WalkState } from "./dispatch-walk.ts";

export type { HookPhase, ToolModule } from "./dispatch-context.ts";
export type { ModuleResult } from "./dispatch-bash.ts";

export type DispatchOptions = {
  /** Built-in modules in the order `mod.sh` globbed them. */
  readonly builtins: readonly ToolModule[];
  /** `plugins/toolu/hooks/lib`, exported to registry bash modules as `TOOLU_LIB_DIR`. */
  readonly libDir: string;
  /** Default `process.env`. */
  readonly env?: HostEnv;
  /** The hook process's working directory. Default `process.cwd()`. */
  readonly cwd?: string;
};

/** `toolu_dispatch_hook`'s fixed reply when `apply_patch` headers do not parse. */
export const MALFORMED_PATCH_DENY =
  '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"Unable to parse apply_patch file headers; patch blocked so protected-file and quality gates cannot be bypassed."}}\n';

/** `toolu_dispatch_hook`'s fixed PostToolUse reply when `apply_patch` headers do not parse. */
export const MALFORMED_PATCH_BLOCK =
  '{"decision":"block","reason":"Unable to parse apply_patch file headers; per-file post-edit quality checks could not run."}\n';

/** The synthetic single-path `Edit` payload `toolu_dispatch_hook` builds with `jq -c`. */
function syntheticEdit(doc: Record<string, unknown>, record: EditRecord): string {
  const input = doc.tool_input;
  const base = input === undefined || input === null || input === false ? {} : input;
  return toJqJson(
    {
      ...doc,
      tool_name: "Edit",
      tool_input: {
        ...(isJsonObject(base) ? base : {}),
        file_path: record.path,
        path: record.path,
        toolu_edit_operation: record.operation,
        toolu_edit_from: record.from ?? "",
        toolu_edit_moved_to: record.moved_to ?? "",
      },
    },
    false,
  );
}

/**
 * Walk `records` one synthetic payload at a time, as `toolu_dispatch_hook`
 * does: sequential by contract, so no path is walked after a deny.
 */
async function dispatchRecords(
  doc: Record<string, unknown>,
  records: readonly EditRecord[],
  at: number,
  walk: { session: Session; builtins: readonly ToolModule[]; state: WalkState },
): Promise<ModuleResult> {
  const record = records[at];
  if (record === undefined) return settle(walk.state);
  const edit = {
    operation: record.operation,
    from: record.from ?? "",
    movedTo: record.moved_to ?? "",
  };
  const payload = { text: syntheticEdit(doc, record), toolName: "Edit", edit };
  const result = await dispatchModules(payload, walk.session, walk.builtins);
  walk.state.stderr.push(result.stderr);
  const done = consume(walk.state, "", {
    ...result,
    stderr: "",
    stdout: substituted(result.stdout),
  });
  return done ?? dispatchRecords(doc, records, at + 1, walk);
}

async function dispatchInput(
  input: string,
  session: Session,
  builtins: readonly ToolModule[],
): Promise<ModuleResult> {
  const doc = parseDocument(input);
  const toolName = readField(doc, ["tool_name"]);
  const normalized = normalizeEditRecords(doc, toolName);
  if (normalized.kind === "not-edit") {
    return dispatchModules({ text: input, toolName }, session, builtins);
  }
  if (normalized.kind === "malformed" || normalized.records.length === 0 || !isJsonObject(doc)) {
    const stdout = session.phase === "pre" ? MALFORMED_PATCH_DENY : MALFORMED_PATCH_BLOCK;
    return { stdout, stderr: "", exitCode: 0 };
  }
  const state = newWalkState(session.phase);
  return dispatchRecords(doc, normalized.records, 0, { session, builtins, state });
}

/**
 * What `mod.sh` adds to the environment before dispatching. Pre: the config
 * root. Post also exports `PROJECT_ROOT` (the cwd's git toplevel, else the cwd)
 * and puts `$PROJECT_ROOT/node_modules/.bin` first on `PATH`.
 */
function sessionFor(
  phase: HookPhase,
  env: HostEnv,
  host: HostName,
  options: DispatchOptions,
): Session {
  const root = configRoot({ env, host });
  const cwd = options.cwd ?? process.cwd();
  const base = { phase, host, configRoot: root, libDir: options.libDir, cwd };
  if (phase === "pre") {
    const project = projectRoot({ env, host, cwd }) ?? cwd;
    return { ...base, env: childEnv(env, { TOOLU_CONFIG_DIR: root }), projectRoot: project };
  }
  const project = gitToplevel(env, cwd) ?? cwd;
  const path = `${project}/node_modules/.bin:${env.PATH ?? ""}`;
  const extra = { TOOLU_CONFIG_DIR: root, PROJECT_ROOT: project, PATH: path };
  return { ...base, env: childEnv(env, extra), projectRoot: project };
}

async function dispatchHook(
  phase: HookPhase,
  stdin: string,
  options: DispatchOptions,
): Promise<ModuleResult> {
  const env = options.env ?? process.env;
  const host = detectHost({ env });
  const warnings: string[] = [];
  const config = loadConfig({
    env,
    host,
    warn: (line) => warnings.push(`toolu-config: ${line}\n`),
  });
  if (!enabled(config, "hooks", phase === "pre" ? "pre-tools" : "post-tools")) {
    return { stdout: "", stderr: warnings.join(""), exitCode: 0 };
  }
  const session = sessionFor(phase, env, host, options);
  const result = await dispatchInput(substituted(stdin), session, options.builtins);
  return { ...result, stderr: warnings.join("") + result.stderr };
}

/** One PreToolUse hook call: the host's stdin in, what the host must see out. */
export function dispatchPreTool(stdin: string, options: DispatchOptions): Promise<ModuleResult> {
  return dispatchHook("pre", stdin, options);
}

/** One PostToolUse hook call: the host's stdin in, what the host must see out. */
export function dispatchPostTool(stdin: string, options: DispatchOptions): Promise<ModuleResult> {
  return dispatchHook("post", stdin, options);
}
