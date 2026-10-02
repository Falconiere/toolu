/**
 * In-process registry runner (#257). Walks one event's registry directory in
 * byte order and imports each active ESM module, the way `dispatch.sh` walks
 * `*.sh`. A module that throws, rejects, fails to import, breaks the contract
 * or returns a non-decision is reported and skipped; the walk goes on. The
 * walk stops after a deny (pre) or a block (post), so later modules and their
 * side effects never run. Merging the returned outcomes is the dispatcher's job.
 */
import { statSync } from "node:fs";
import { join } from "node:path";
import { inspect } from "node:util";
import { z } from "zod";
import { isJsonObject } from "../config/config-load.ts";
import { DecisionSchema, type Decision } from "../decision/decision.ts";
import { listRegistryDir, type RegistryEntry } from "./registry-list.ts";
import { pluginActive } from "./registry-gate.ts";
import { registryDirName } from "./registry-paths.ts";
import {
  REGISTRY_EVENTS,
  registryEventFor,
  type RegistryContext,
  type RegistryHookEvent,
  type RegistryModule,
} from "./registry-types.ts";

export type ModuleOutcome =
  | { entry: RegistryEntry; status: "decision"; decision: Decision; ms: number }
  | { entry: RegistryEntry; status: "error"; error: string; ms: number }
  | { entry: RegistryEntry; status: "skipped"; reason: "inactive" | "bash" | "shadowed" };

/** Runs a `.sh` registry entry, e.g. through the bash bridge until its port lands (#258). */
export type BashFallback = (
  entry: RegistryEntry,
  event: RegistryHookEvent,
  ctx: RegistryContext,
) => Promise<Decision>;

export type RunRegistryOptions = {
  fallback?: BashFallback;
  warn?: (line: string) => void;
  /** Host-resolved selection; absent preserves installed-plugin gating alone. */
  selectedSpecs?: ReadonlySet<string>;
};

function stderrLine(line: string): void {
  process.stderr.write(`${line}\n`);
}

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

const ModuleSchema = z.looseObject({
  spec: z.string(),
  name: z.string(),
  event: z.enum(REGISTRY_EVENTS),
  run: z.custom<RegistryModule["run"]>((value) => typeof value === "function"),
});

/** Run the module's default export after checking it against the file it came from. */
function runContract(
  loaded: unknown,
  entry: RegistryEntry,
  event: RegistryHookEvent,
  ctx: RegistryContext,
): Promise<unknown> {
  const exported = isJsonObject(loaded) ? loaded.default : undefined;
  const parsed = ModuleSchema.safeParse(exported);
  if (!parsed.success) throw new Error("default export is not a registry module");
  const { spec, name, event: declared } = parsed.data;
  const want = { spec: entry.spec, name: entry.name, event: registryEventFor(event.type) };
  if (spec !== want.spec || name !== want.name || declared !== want.event) {
    throw new Error(
      `contract mismatch: exports ${JSON.stringify({ spec, name, event: declared })}, file and directory want ${JSON.stringify(want)}`,
    );
  }
  // Called on the export itself, not zod's copy: the copy keeps own properties
  // only, so a class-instance module would lose its prototype methods on `this`.
  return parsed.data.run.call(exported, event, ctx);
}

/**
 * Import `entry` fresh whenever its bytes may have changed, so a long-lived
 * process sees a re-registered module. Bun keys its module cache on the query of
 * a plain path specifier but ignores it on a `file://` URL, hence the bare path.
 */
async function importModule(entry: RegistryEntry): Promise<unknown> {
  const stat = statSync(entry.path);
  const loaded: unknown = await import(
    `${entry.path}?v=${String(stat.mtimeMs)}-${String(stat.size)}`
  );
  return loaded;
}

async function runEsm(
  entry: RegistryEntry,
  event: RegistryHookEvent,
  ctx: RegistryContext,
): Promise<Decision> {
  const result = await runContract(await importModule(entry), entry, event, ctx);
  const decision = DecisionSchema.safeParse(result);
  if (!decision.success) {
    throw new Error(`invalid decision: ${inspect(result)}`);
  }
  return decision.data;
}

async function timed(entry: RegistryEntry, work: () => Promise<Decision>): Promise<ModuleOutcome> {
  const start = performance.now();
  try {
    const decision = await work();
    return { entry, status: "decision", decision, ms: performance.now() - start };
  } catch (error) {
    return { entry, status: "error", error: message(error), ms: performance.now() - start };
  }
}

/** A deny before the tool runs, or a block after it, ends the walk as in `dispatch.sh`. */
function stops(event: RegistryHookEvent, outcome: ModuleOutcome): boolean {
  if (outcome.status !== "decision") return false;
  const kind = event.type === "tool/post" ? "post_block" : "deny";
  return outcome.decision.kind === kind;
}

type Walk = {
  warn: (line: string) => void;
  event: RegistryHookEvent;
  ctx: RegistryContext;
  fallback: BashFallback | undefined;
  esmSpecs: ReadonlySet<string>;
  active: (spec: string) => boolean;
};

async function outcomeOf(entry: RegistryEntry, walk: Walk): Promise<ModuleOutcome> {
  const { event, ctx, fallback } = walk;
  if (!walk.active(entry.spec)) return { entry, status: "skipped", reason: "inactive" };
  if (entry.kind === "esm") return timed(entry, () => runEsm(entry, event, ctx));
  if (walk.esmSpecs.has(entry.spec)) return { entry, status: "skipped", reason: "shadowed" };
  if (fallback === undefined) return { entry, status: "skipped", reason: "bash" };
  return timed(entry, () => fallback(entry, event, ctx));
}

function memoActive(
  ctx: RegistryContext,
  selectedSpecs: ReadonlySet<string> | undefined,
): (spec: string) => boolean {
  const memo = new Map<string, boolean>();
  return (spec) => {
    const known = memo.get(spec);
    if (known !== undefined) return known;
    const active =
      (selectedSpecs === undefined || selectedSpecs.has(spec)) &&
      pluginActive(spec, { env: ctx.env, host: ctx.host });
    memo.set(spec, active);
    return active;
  };
}

/** Sequential by contract: order is observable through each module's side effects. */
async function walkFrom(
  entries: readonly RegistryEntry[],
  at: number,
  walk: Walk,
  outcomes: ModuleOutcome[],
): Promise<ModuleOutcome[]> {
  const entry = entries[at];
  if (entry === undefined) return outcomes;
  const outcome = await outcomeOf(entry, walk);
  outcomes.push(outcome);
  if (outcome.status === "error") {
    walk.warn(`toolu-registry: module ${entry.file} failed: ${outcome.error}; output skipped`);
  }
  return stops(walk.event, outcome) ? outcomes : walkFrom(entries, at + 1, walk, outcomes);
}

/**
 * Run every registry module for `event` from `<ctx.configRoot>/toolu/<dir>.d`,
 * in order, and return one outcome per module reached.
 */
export async function runRegistry(
  event: RegistryHookEvent,
  ctx: RegistryContext,
  options: RunRegistryOptions = {},
): Promise<ModuleOutcome[]> {
  const warn = options.warn ?? stderrLine;
  const dir = join(ctx.configRoot, "toolu", registryDirName(registryEventFor(event.type)));
  const { entries, rejected } = listRegistryDir(dir);
  for (const file of rejected) {
    warn(`toolu-registry: registry module ${file} lacks <plugin-spec>__<name> namespace; skipped`);
  }
  const walk: Walk = {
    warn,
    event,
    ctx,
    fallback: options.fallback,
    esmSpecs: new Set(entries.filter((e) => e.kind === "esm").map((e) => e.spec)),
    active: memoActive(ctx, options.selectedSpecs),
  };
  return walkFrom(entries, 0, walk, []);
}
