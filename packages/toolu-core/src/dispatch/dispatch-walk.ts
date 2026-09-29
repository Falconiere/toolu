/**
 * One module walk over one payload, a port of `toolu_dispatch_modules`:
 * built-ins in table order, then every registry module (#257). A module exit
 * of 2 ends the walk with that module's stderr; any other non-zero exit is
 * reported and skipped; advisories are merged. PreToolUse (#258): the first
 * deny is emitted verbatim and ends the walk, the first ask is held.
 * PostToolUse (#259): the first `decision: "block"` is emitted verbatim and
 * ends the walk; `permissionDecision` means nothing there.
 */
import { DecisionSchema, type Decision } from "../decision/decision.ts";
import { encodeDecision } from "../host/host-encode.ts";
import type { HostName } from "../host/host-name.ts";
import { runRegistry, type BashFallback, type ModuleOutcome } from "../registry/registry-run.ts";
import type { RegistryContext, RegistryHookEvent } from "../registry/registry-types.ts";
import { childEnv, moduleStdin, runBash, type ModuleResult } from "./dispatch-bash.ts";
import type { HookPhase, Payload, Session, ToolModule } from "./dispatch-context.ts";
import { toolContext, toolEvent } from "./dispatch-context.ts";
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

export type WalkState = {
  readonly phase: HookPhase;
  advisories: Advisories;
  ask: string | undefined;
  stderr: string[];
};

export function newWalkState(phase: HookPhase): WalkState {
  return { phase, advisories: emptyAdvisories(), ask: undefined, stderr: [] };
}

/** `.hookSpecificOutput.permissionDecision // empty` of a result. */
export function permissionOf(doc: unknown): string {
  return readField(doc, ["hookSpecificOutput", "permissionDecision"]);
}

/** Whether a module's parsed result ends the walk: a deny before the tool, a block after it. */
function stopsWalk(phase: HookPhase, doc: unknown): boolean {
  return phase === "pre" ? permissionOf(doc) === "deny" : readField(doc, ["decision"]) === "block";
}

/**
 * Fold one module's result into the walk. Returns the hook's final result when
 * this module ends the walk (deny, block or exit 2), otherwise undefined.
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
  if (stopsWalk(state.phase, doc)) {
    return { stdout: printed(result.stdout), stderr: state.stderr.join(""), exitCode: 0 };
  }
  if (state.phase === "pre" && permissionOf(doc) === "ask") {
    state.ask ??= result.stdout;
    return undefined;
  }
  collectAdvisories(state.advisories, doc);
  return undefined;
}

/** The walk's result once every module ran: the held ask, else the merged advisories. */
export function settle(state: WalkState): ModuleResult {
  const event = state.phase === "pre" ? "PreToolUse" : "PostToolUse";
  const stdout =
    state.ask === undefined
      ? finalAdvisory(state.advisories, event)
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
function encoded(
  host: HostName,
  type: RegistryHookEvent["type"],
  decision: Decision,
): ModuleResult {
  const target = host === "codex" ? "codex" : "claude";
  const out = encodeDecision(target, type, decision);
  return { stdout: out.kind === "command" ? substituted(out.stdout) : "", stderr: "", exitCode: 0 };
}

async function runNative(
  module: ToolModule,
  event: RegistryHookEvent,
  ctx: RegistryContext,
): Promise<ModuleResult> {
  try {
    const parsed = DecisionSchema.safeParse(await module.run(event, ctx));
    if (parsed.success) return encoded(ctx.host, event.type, parsed.data);
    return { stdout: "", stderr: "", exitCode: 1 };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { stdout: "", stderr: `${message}\n`, exitCode: 1 };
  }
}

type Walk = { payload: Payload; session: Session; event: RegistryHookEvent; ctx: RegistryContext };

/** Sequential by contract: a module must not run after a deny. */
async function walkBuiltins(
  builtins: readonly ToolModule[],
  at: number,
  walk: Walk,
  state: WalkState,
): Promise<ModuleResult | undefined> {
  const module = builtins[at];
  if (module === undefined) return undefined;
  const done = consume(state, module.name, await runNative(module, walk.event, walk.ctx));
  return done ?? walkBuiltins(builtins, at + 1, walk, state);
}

/**
 * Registry `.sh` modules run on bash; their raw result is kept for the merge.
 * The decision only tells the registry runner whether to stop.
 */
function bashFallback(walk: Walk, raw: Map<string, ModuleResult>): BashFallback {
  return (entry) => {
    const result = runBash(
      entry.path,
      moduleStdin(walk.payload.text),
      moduleEnv(walk.payload, walk.session),
    );
    raw.set(entry.path, result);
    const phase = walk.session.phase;
    const stops =
      result.exitCode === 2 ||
      (result.exitCode === 0 && stopsWalk(phase, parseDocument(result.stdout)));
    const decision: Decision = !stops
      ? { kind: "allow" }
      : phase === "pre"
        ? { kind: "deny", reason: "registry module denied" }
        : { kind: "post_block", reason: "registry module blocked" };
    return Promise.resolve(decision);
  };
}

function outcomeResult(
  outcome: ModuleOutcome,
  walk: Walk,
  raw: Map<string, ModuleResult>,
): ModuleResult | undefined {
  if (outcome.status !== "decision") return undefined;
  if (outcome.entry.kind === "esm")
    return encoded(walk.ctx.host, walk.event.type, outcome.decision);
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

/** `toolu_dispatch_modules <modules> <PreToolUse|PostToolUse> <registry>` over one payload. */
export async function dispatchModules(
  payload: Payload,
  session: Session,
  builtins: readonly ToolModule[],
): Promise<ModuleResult> {
  const doc = parseDocument(payload.text);
  const walk: Walk = {
    payload,
    session,
    event: toolEvent(payload, doc, session),
    ctx: toolContext(payload, doc, session),
  };
  const state = newWalkState(session.phase);
  const done = (await walkBuiltins(builtins, 0, walk, state)) ?? (await walkRegistry(walk, state));
  return done ?? settle(state);
}
