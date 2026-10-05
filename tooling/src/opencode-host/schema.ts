/**
 * Zod contracts for the OpenCode host contract (#335): the pin, the committed
 * live-probe results and the 12-plugin capability matrix, all under
 * `tools/toolu-opencode/contract/`. Strict objects: unknown keys fail.
 */
import { readFileSync } from "node:fs";
import { z } from "zod";

export const PROBE_IDS = [
  "load.local-file",
  "load.config-file",
  "load.module-default",
  "load.init-throw",
  "load.helper-export",
  "deny.bash",
  "deny.write",
  "deny.edit",
  "deny.grep",
  "deny.apply-patch",
  "deny.mcp",
  "deny.task-child",
  "pre.advisory",
  "permission.ask-hook",
  "permission.config-deny",
  "permission.order",
  "post.feedback",
  "post.bash-exit",
  "post.tool-error",
  "context.system",
  "context.prompt",
  "context.compaction",
  "env.shell",
  "command.hook",
  "surface.files",
  "surface.names",
  "surface.config-hook",
  "ui.toast",
  "events.bus",
] as const;
export const ProbeId = z.enum(PROBE_IDS);
export type ProbeId = z.infer<typeof ProbeId>;

export const AXES = [
  "tools",
  "permission",
  "startup",
  "prompt",
  "compaction",
  "postTool",
  "mcp",
  "task",
  "ui",
] as const;
export const MatrixAxis = z.enum(AXES);
export type MatrixAxis = z.infer<typeof MatrixAxis>;
const ProbeAxis = z.enum([...AXES, "load", "surfaces", "env"]);

export const Verdict = z.enum(["supported", "unsupported"]);
export type Verdict = z.infer<typeof Verdict>;

export const PinSchema = z.strictObject({
  version: z.literal(1),
  cli: z.strictObject({
    package: z.literal("opencode-ai"),
    version: z.string().regex(/^\d+\.\d+\.\d+$/),
  }),
  sdk: z.strictObject({
    package: z.literal("@opencode-ai/plugin"),
    version: z.string().regex(/^\d+\.\d+\.\d+$/),
  }),
  docs: z.url(),
});
export type Pin = z.infer<typeof PinSchema>;

const ProbeResultSchema = z.strictObject({
  id: ProbeId,
  axis: ProbeAxis,
  kind: z.enum(["hook", "event", "loader", "config", "surface"]),
  mechanism: z.string().min(1),
  claim: z.string().min(1),
  verdict: Verdict,
  observed: z.record(z.string(), z.union([z.boolean(), z.string(), z.number()])),
});
export type ProbeResult = z.infer<typeof ProbeResultSchema>;

export const ProbeResultsSchema = z.strictObject({
  version: z.literal(1),
  recordedAt: z.string().regex(/^\d{4}-\d{2}-\d{2}$/),
  host: z.strictObject({
    cli: z.string(),
    cliVersion: z.string(),
    sdk: z.string(),
    provisionedSdkVersion: z.string(),
    platform: z.string(),
    bun: z.string(),
    installSource: z.string(),
  }),
  probes: z.array(ProbeResultSchema),
});
export type ProbeResults = z.infer<typeof ProbeResultsSchema>;

/** Epic #334 work packages OP-02…OP-29; OP-n is issue #(334 + n). */
export const WorkPackage = z.string().regex(/^OP-(0[2-9]|1\d|2\d)$/);
export type WorkPackage = z.infer<typeof WorkPackage>;

export function workPackageIssue(wp: WorkPackage): number {
  return 334 + Number(wp.slice(3));
}

const UsedCell = z.strictObject({
  use: z.string().min(1),
  required: z.boolean(),
  enforcement: z.boolean(),
  mechanism: z.string().min(1),
  kind: z.enum(["hook", "event", "config", "tool", "tui"]),
  status: z.enum(["supported", "partial", "unsupported"]),
  evidence: z.array(ProbeId).min(1),
  owner: z.array(WorkPackage).min(1),
  alternative: z.string().min(1).optional(),
  alternativeEvidence: z.array(ProbeId).min(1).optional(),
  releaseBlocker: z.boolean().optional(),
});
const CellSchema = z.union([z.strictObject({ use: z.literal("none") }), UsedCell]);
export type Cell = z.infer<typeof CellSchema>;
export type UsedCell = z.infer<typeof UsedCell>;

export function isUsed(cell: Cell): cell is UsedCell {
  return cell.use !== "none";
}

/** One verdict per probe id; zod 4 records over an enum key are exhaustive. */
export const VerdictsSchema = z.record(ProbeId, Verdict);
export type Verdicts = z.infer<typeof VerdictsSchema>;

const PluginRowSchema = z.strictObject({
  owner: z.array(WorkPackage).min(1),
  axes: z.strictObject({
    tools: CellSchema,
    permission: CellSchema,
    startup: CellSchema,
    prompt: CellSchema,
    compaction: CellSchema,
    postTool: CellSchema,
    mcp: CellSchema,
    task: CellSchema,
    ui: CellSchema,
  }),
  surfaces: z.strictObject({
    skills: z.number().int().nonnegative(),
    commands: z.number().int().nonnegative(),
    agents: z.number().int().nonnegative(),
    owner: z.array(WorkPackage).min(1),
  }),
  notes: z.array(z.strictObject({ need: z.string().min(1), owner: WorkPackage })).optional(),
});

/** A host-wide requirement on the adapter itself, e.g. a loader constraint. */
const HostConstraintSchema = z.strictObject({
  need: z.string().min(1),
  evidence: z.array(ProbeId).min(1),
  owner: z.array(WorkPackage).min(1),
});

export const MatrixSchema = z.strictObject({
  version: z.literal(1),
  host: z.array(HostConstraintSchema),
  plugins: z.record(z.string(), PluginRowSchema),
});
export type Matrix = z.infer<typeof MatrixSchema>;

export class ContractError extends Error {}

/** Parse a JSON file against `schema`, naming the file in every failure. */
export function readJson<T>(path: string, schema: z.ZodType<T>): T {
  let raw: unknown;
  try {
    raw = JSON.parse(readFileSync(path, "utf8"));
  } catch (err: unknown) {
    throw new ContractError(`${path}: ${err instanceof Error ? err.message : String(err)}`);
  }
  const parsed = schema.safeParse(raw);
  if (!parsed.success)
    throw new ContractError(`${path}: ${z.prettifyError(parsed.error).replaceAll("\n", " ")}`);
  return parsed.data;
}
