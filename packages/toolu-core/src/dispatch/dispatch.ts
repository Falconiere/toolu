/**
 * `@toolu/core/dispatch` (#258): the PreToolUse dispatcher, a TypeScript port
 * of `pre-tools/mod.sh` + `toolu_dispatch_hook`. An edit tool is walked once
 * per affected path (a deny or exit 2 on any path wins for the whole patch);
 * everything else is walked once. Module results stay raw hook text, so the
 * output matches the bash dispatcher byte for byte while modules still run on
 * bash, and each module cuts over to native TypeScript on its own.
 */
import { isJsonObject, loadConfig } from "../config/config-load.ts";
import { enabled } from "../config/config-read.ts";
import { detectHost } from "../host/host-detect.ts";
import type { HostEnv } from "../host/host-name.ts";
import { configRoot, projectRoot } from "../host/host-roots.ts";
import { normalizeEditRecords } from "../state/edit-records.ts";
import type { EditRecord } from "../state/state-schema.ts";
import { toJqJson } from "../state/state-io.ts";
import { childEnv, shellStatus, type ModuleResult } from "./dispatch-bash.ts";
import type { PreToolModule, Session } from "./dispatch-context.ts";
import { parseDocument, readField, substituted } from "./dispatch-output.ts";
import { consume, dispatchModules, newWalkState, settle, type WalkState } from "./dispatch-walk.ts";

export { bashModule, type PreToolModule } from "./dispatch-context.ts";
export type { ModuleResult } from "./dispatch-bash.ts";

export type DispatchOptions = {
  /** Built-in modules in the order `mod.sh` globbed them. */
  readonly builtins: readonly PreToolModule[];
  /** `plugins/toolu/hooks/lib`, exported to bash modules as `TOOLU_LIB_DIR`. */
  readonly libDir: string;
  /** Default `process.env`. */
  readonly env?: HostEnv;
};

/** `toolu_dispatch_hook`'s fixed reply when `apply_patch` headers do not parse. */
export const MALFORMED_PATCH_DENY =
  '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"Unable to parse apply_patch file headers; patch blocked so protected-file and quality gates cannot be bypassed."}}\n';

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
  walk: { session: Session; builtins: readonly PreToolModule[]; state: WalkState },
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
  builtins: readonly PreToolModule[],
): Promise<ModuleResult> {
  const doc = parseDocument(input);
  const toolName = readField(doc, ["tool_name"]);
  const normalized = normalizeEditRecords(doc, toolName);
  if (normalized.kind === "not-edit") {
    return dispatchModules({ text: input, toolName }, session, builtins);
  }
  if (normalized.kind === "malformed" || normalized.records.length === 0 || !isJsonObject(doc)) {
    return { stdout: MALFORMED_PATCH_DENY, stderr: "", exitCode: 0 };
  }
  return dispatchRecords(doc, normalized.records, 0, { session, builtins, state: newWalkState() });
}

/** One PreToolUse hook call: the host's stdin in, what the host must see out. */
export async function dispatchPreTool(
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
  if (!enabled(config, "hooks", "pre-tools")) {
    return { stdout: "", stderr: warnings.join(""), exitCode: 0 };
  }
  const root = configRoot({ env, host });
  const session: Session = {
    host,
    env: childEnv(env, { TOOLU_CONFIG_DIR: root }),
    configRoot: root,
    projectRoot: projectRoot({ env, host }) ?? process.cwd(),
    libDir: options.libDir,
  };
  const result = await dispatchInput(substituted(stdin), session, options.builtins);
  return { ...result, stderr: warnings.join("") + result.stderr };
}

/**
 * A standalone hook script run as the host would run it (#258): exec'd through
 * its shebang with the host's stdin and environment, its bytes relayed as is.
 */
export function runScript(script: string, stdin: string, env: HostEnv = process.env): ModuleResult {
  try {
    const proc = Bun.spawnSync([script], {
      stdin: Buffer.from(stdin),
      stdout: "pipe",
      stderr: "pipe",
      env: childEnv(env, {}),
    });
    return {
      stdout: proc.stdout.toString(),
      stderr: proc.stderr.toString(),
      exitCode: shellStatus(proc),
    };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { stdout: "", stderr: `toolu: cannot run ${script}: ${message}\n`, exitCode: 2 };
  }
}
