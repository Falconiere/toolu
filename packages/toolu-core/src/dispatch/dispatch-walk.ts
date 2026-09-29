/**
 * One module walk over one payload (#258), a port of `toolu_dispatch_modules`
 * under PreToolUse semantics: built-ins in table order, then every registry
 * module (#257). The first deny is emitted verbatim and ends the walk; a
 * module exit of 2 ends it with that module's stderr; any other non-zero exit is
 * reported and skipped; the first ask is held; advisories are merged.
 */
import { DecisionSchema, type Decision } from "../decision/decision.ts";
import { encodeDecision } from "../host/host-encode.ts";
import type { HostName } from "../host/host-name.ts";
import { runRegistry, type BashFallback, type ModuleOutcome } from "../registry/registry-run.ts";
import type { RegistryContext, RegistryHookEvent } from "../registry/registry-types.ts";
import { childEnv, moduleStdin, runBash, type ModuleResult } from "./dispatch-bash.ts";
import type { Payload, PreToolModule, Session } from "./dispatch-context.ts";
import { preToolContext, preToolEvent } from "./dispatch-context.ts";
import {
  collectAdvisories,
  emptyAdvisories,
  finalAdvisory,
  finalAsk,
  parseDocument,
  printed,
  readField,
  substituted,
  type Advisories,
} from "./dispatch-output.ts";

export type WalkState = { advisories: Advisories; ask: string | undefined; stderr: string[] };

export function newWalkState(): WalkState {
  return { advisories: emptyAdvisories(), ask: undefined, stderr: [] };
}

/** `.hookSpecificOutput.permissionDecision // empty` of a result. */
export function permissionOf(doc: unknown): string {
  return readField(doc, ["hookSpecificOutput", "permissionDecision"]);
}

/**
 * Fold one module's result into the walk. Returns the hook's final result when
 * this module ends the walk (deny or exit 2), otherwise undefined.
 */
export function consume(
  state: WalkState,
  name: string,
  result: ModuleResult,
): ModuleResult | undefined {
  if (result.exitCode === 2) {
    return { stdout: "", stderr: state.stderr.join("") + result.stderr, exitCode: 2 };
  }
  if (result.exitCode !== 0) {
    state.stderr.push(
      `toolu-dispatch: module ${name} exited ${String(result.exitCode)}; output skipped\n`,
    );
    return undefined;
  }
  if (result.stdout === "") return undefined;
  const doc = parseDocument(result.stdout);
  const permission = permissionOf(doc);
  if (permission === "deny") {
    return { stdout: printed(result.stdout), stderr: state.stderr.join(""), exitCode: 0 };
  }
  if (permission === "ask") {
    state.ask ??= result.stdout;
    return undefined;
  }
  collectAdvisories(state.advisories, doc);
  return undefined;
}

/** The walk's result once every module ran: the held ask, else the merged advisories. */
export function settle(state: WalkState): ModuleResult {
  const stdout =
    state.ask === undefined
      ? finalAdvisory(state.advisories)
      : finalAsk(state.ask, state.advisories);
  return { stdout, stderr: state.stderr.join(""), exitCode: 0 };
}

/** The environment `mod.sh` exports to its modules for this payload. */
function moduleEnv(payload: Payload, session: Session): Record<string, string> {
  const extra: Record<string, string> = {
    input: payload.text,
    tool_name: payload.toolName,
    TOOLU_LIB_DIR: session.libDir,
    TOOLU_CONFIG_DIR: session.configRoot,
  };
  if (payload.edit !== undefined) {
    extra.TOOLU_EDIT_OPERATION = payload.edit.operation;
    extra.TOOLU_EDIT_FROM = payload.edit.from;
    extra.TOOLU_EDIT_MOVED_TO = payload.edit.movedTo;
  }
  return childEnv(session.env, extra);
}

/** A native decision as the hook JSON a bash module would have printed. */
function encoded(host: HostName, decision: Decision): ModuleResult {
  const target = host === "codex" ? "codex" : "claude";
  const out = encodeDecision(target, "tool/pre", decision);
  return { stdout: out.kind === "command" ? substituted(out.stdout) : "", stderr: "", exitCode: 0 };
}

async function runNative(
  module: Extract<PreToolModule, { kind: "native" }>,
  event: RegistryHookEvent,
  ctx: RegistryContext,
): Promise<ModuleResult> {
  try {
    const decision = DecisionSchema.safeParse(await module.run(event, ctx));
    if (decision.success) return encoded(ctx.host, decision.data);
    return { stdout: "", stderr: "", exitCode: 1 };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { stdout: "", stderr: `${message}\n`, exitCode: 1 };
  }
}

type Walk = { payload: Payload; session: Session; event: RegistryHookEvent; ctx: RegistryContext };

async function runBuiltin(module: PreToolModule, walk: Walk): Promise<ModuleResult> {
  if (module.kind === "native") return runNative(module, walk.event, walk.ctx);
  return runBash(
    module.path,
    moduleStdin(walk.payload.text),
    moduleEnv(walk.payload, walk.session),
  );
}

/** Sequential by contract: a module must not run after a deny. */
async function walkBuiltins(
  builtins: readonly PreToolModule[],
  at: number,
  walk: Walk,
  state: WalkState,
): Promise<ModuleResult | undefined> {
  const module = builtins[at];
  if (module === undefined) return undefined;
  const done = consume(state, module.name, await runBuiltin(module, walk));
  return done ?? walkBuiltins(builtins, at + 1, walk, state);
}

/** Registry `.sh` modules run on bash; their raw result is kept for the merge. */
function bashFallback(walk: Walk, raw: Map<string, ModuleResult>): BashFallback {
  return (entry) => {
    const result = runBash(
      entry.path,
      moduleStdin(walk.payload.text),
      moduleEnv(walk.payload, walk.session),
    );
    raw.set(entry.path, result);
    const stops =
      result.exitCode === 2 ||
      (result.exitCode === 0 && permissionOf(parseDocument(result.stdout)) === "deny");
    const decision: Decision = stops
      ? { kind: "deny", reason: "registry module denied" }
      : { kind: "allow" };
    return Promise.resolve(decision);
  };
}

function outcomeResult(
  outcome: ModuleOutcome,
  walk: Walk,
  raw: Map<string, ModuleResult>,
): ModuleResult | undefined {
  if (outcome.status !== "decision") return undefined;
  if (outcome.entry.kind === "esm") return encoded(walk.ctx.host, outcome.decision);
  return raw.get(outcome.entry.path);
}

async function walkRegistry(walk: Walk, state: WalkState): Promise<ModuleResult | undefined> {
  const raw = new Map<string, ModuleResult>();
  const outcomes = await runRegistry(walk.event, walk.ctx, {
    fallback: bashFallback(walk, raw),
    warn: (line) => state.stderr.push(`${line}\n`),
  });
  for (const outcome of outcomes) {
    const result = outcomeResult(outcome, walk, raw);
    const done = result === undefined ? undefined : consume(state, outcome.entry.file, result);
    if (done !== undefined) return done;
  }
  return undefined;
}

/** `toolu_dispatch_modules <modules> PreToolUse <registry>` over one payload. */
export async function dispatchModules(
  payload: Payload,
  session: Session,
  builtins: readonly PreToolModule[],
): Promise<ModuleResult> {
  const doc = parseDocument(payload.text);
  const walk: Walk = {
    payload,
    session,
    event: preToolEvent(payload, doc, session),
    ctx: preToolContext(payload, doc, session),
  };
  const state = newWalkState();
  const done = (await walkBuiltins(builtins, 0, walk, state)) ?? (await walkRegistry(walk, state));
  return done ?? settle(state);
}
