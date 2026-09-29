/**
 * Module contract and per-payload context for the PreToolUse dispatcher (#258).
 * Built-in modules share `run(event, ctx)` with #257's `RegistryModule`, so a
 * port swaps one `bashModule(...)` for a native module without touching the
 * walk. Every context comes from `preToolContext`: #284 adds its lazily parsed
 * shell command there, as an optional `RegistryContext` field.
 */
import { join } from "node:path";
import { isJsonObject } from "../config/config-load.ts";
import type { Decision } from "../decision/decision.ts";
import type { HostEnv, HostName } from "../host/host-name.ts";
import type { RegistryContext, RegistryHookEvent } from "../registry/registry-types.ts";
import type { EditRecord } from "../state/state-schema.ts";

/** A built-in module: a bash script until its port lands, then native TypeScript. */
export type PreToolModule =
  | { readonly kind: "bash"; readonly name: string; readonly path: string }
  | {
      readonly kind: "native";
      readonly name: string;
      run(event: RegistryHookEvent, ctx: RegistryContext): Promise<Decision>;
    };

/** `modules/<name>.sh` run through bash; the fallback for a module not yet ported. */
export function bashModule(modulesDir: string, name: string): PreToolModule {
  return { kind: "bash", name: `${name}.sh`, path: join(modulesDir, `${name}.sh`) };
}

/** An edit split out of a multi-file patch: the `TOOLU_EDIT_*` a bash module sees. */
export type EditSplit = { operation: EditRecord["operation"]; from: string; movedTo: string };

/** One walk's inputs: the payload text every module reads, plus what it was built from. */
export type Payload = {
  readonly text: string;
  readonly toolName: string;
  readonly edit?: EditSplit;
};

/** What stays fixed across every walk of one hook call. */
export type Session = {
  readonly host: HostName;
  readonly env: HostEnv;
  readonly configRoot: string;
  readonly projectRoot: string;
  readonly libDir: string;
};

function text(value: unknown, fallback: string): string {
  return typeof value === "string" && value !== "" ? value : fallback;
}

/** The normalized event for `payload`; a Bash/Shell call with a command is `shell/pre`. */
export function preToolEvent(payload: Payload, doc: unknown, session: Session): RegistryHookEvent {
  const raw = isJsonObject(doc) ? doc : {};
  const input = isJsonObject(raw.tool_input) ? raw.tool_input : {};
  const cwd = text(raw.cwd, session.projectRoot);
  const base = {
    sessionId: text(raw.session_id, "unknown"),
    cwd,
    projectRoot: session.projectRoot,
    worktree: session.projectRoot,
    toolCallId: text(raw.tool_use_id, "unknown"),
    toolName: text(payload.toolName, "unknown"),
    toolInput: input,
  };
  const command = input.command;
  if (
    (payload.toolName === "Bash" || payload.toolName === "Shell") &&
    typeof command === "string" &&
    command !== ""
  ) {
    return { ...base, type: "shell/pre", command };
  }
  return { ...base, type: "tool/pre" };
}

/** The context a module sees for `payload`. */
export function preToolContext(payload: Payload, doc: unknown, session: Session): RegistryContext {
  const edit = payload.edit;
  return {
    host: session.host,
    env: session.env,
    configRoot: session.configRoot,
    projectRoot: session.projectRoot,
    raw: isJsonObject(doc) ? doc : {},
    ...(edit === undefined ? {} : { edit }),
  };
}
