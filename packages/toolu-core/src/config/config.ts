/** toolu.config.json v1 strict Zod (#210). */
import { z } from "zod";

const GateModeSchema = z.enum(["block", "ask", "advise", "off"]);
const GatePresetSchema = z.enum(["strict", "balanced", "relaxed"]);
const DocsSyncModeSchema = z.enum(["advise", "block", "off"]);
const AgentTierModeSchema = z.enum(["advise", "block", "off"]);
const ModelClassSchema = z.enum(["haiku", "sonnet", "opus", "fable", "inherit"]);
const ReasoningEffortSchema = z.enum(["low", "medium", "high", "xhigh", "max", "ultra"]);

const GateEntrySchema = z.object({ mode: GateModeSchema }).strict();

const LangEntrySchema = z
  .object({
    maxFileLines: z.number().int().positive().optional(),
    maxFnLines: z.number().int().positive().optional(),
    maxImplLines: z.number().int().positive().optional(),
    noMocks: z.boolean().optional(),
  })
  .strict();

const CodexModelEntrySchema = z
  .object({
    model: z.string().min(1),
    reasoningEffort: ReasoningEffortSchema.optional(),
  })
  .strict();

export const TooluConfigSchema = z
  .object({
    version: z.literal(1),
    skills: z.record(z.string(), z.boolean()).optional(),
    hooks: z.record(z.string(), z.boolean()).optional(),
    mcp: z.record(z.string(), z.boolean()).optional(),
    agents: z.record(z.string(), z.boolean()).optional(),
    models: z
      .object({
        enabled: z.boolean().optional(),
        mechanical: ModelClassSchema.optional(),
        exploration: ModelClassSchema.optional(),
        implementation: ModelClassSchema.optional(),
        review: ModelClassSchema.optional(),
        synthesis: ModelClassSchema.optional(),
        architecture: ModelClassSchema.optional(),
        codex: z.record(z.string(), CodexModelEntrySchema).optional(),
      })
      .strict()
      .optional(),
    lang: z
      .object({
        ts: LangEntrySchema.optional(),
        rust: LangEntrySchema.optional(),
        python: LangEntrySchema.optional(),
      })
      .strict()
      .optional(),
    docsSync: z
      .object({
        mode: DocsSyncModeSchema.optional(),
        surfaces: z.array(z.string()).optional(),
        surfaceExcludes: z.array(z.string()).optional(),
        codeSurfaces: z.array(z.string()).optional(),
      })
      .strict()
      .optional(),
    telemetry: z.object({ enabled: z.boolean().optional() }).strict().optional(),
    agentTier: z.object({ mode: AgentTierModeSchema.optional() }).strict().optional(),
    planLedger: z.object({ blockOnUncoveredAcs: z.boolean().optional() }).strict().optional(),
    gates: z
      .object({
        preset: GatePresetSchema.optional(),
        pushReview: GateEntrySchema.optional(),
        qualityGate: GateEntrySchema.optional(),
        commitGate: GateEntrySchema.optional(),
        bashCommands: GateEntrySchema.optional(),
        planLedger: GateEntrySchema.optional(),
        docsSync: GateEntrySchema.optional(),
        agentTier: GateEntrySchema.optional(),
        protectedFiles: GateEntrySchema.optional(),
        mcpBlocker: GateEntrySchema.optional(),
        sweep: z.boolean().optional(),
        stateTtlHours: z.number().int().positive().optional(),
        telemetryRetentionDays: z.number().int().positive().optional(),
      })
      .strict()
      .optional(),
    permissions: z
      .object({
        autoAllow: z.boolean().optional(),
        allow: z.array(z.string()).optional(),
        deny: z.array(z.string()).optional(),
      })
      .strict()
      .optional(),
    projectSkills: z
      .object({
        enabled: z.boolean().optional(),
        staleAfterDays: z.number().int().positive().optional(),
        archiveAfterDays: z.number().int().positive().optional(),
        indexCap: z.number().int().positive().optional(),
      })
      .strict()
      .optional(),
  })
  .strict();

export type TooluConfig = z.infer<typeof TooluConfigSchema>;

/** Parse toolu.config.json; rejects unknown top-level keys. */
export function parseTooluConfig(input: unknown): TooluConfig {
  return TooluConfigSchema.parse(input);
}
