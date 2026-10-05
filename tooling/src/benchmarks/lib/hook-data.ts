/**
 * The hook bench's data files and result (#410): fixed payloads, budgets, the
 * measurer's per-spawn report and the `toolu.hook-resources/v1` result. Every
 * schema is strict, so a typo fails the run instead of being ignored.
 */
import { readFileSync } from "node:fs";
import { z } from "zod";

/** A usage or setup problem (exit 2), or a failed assertion (exit 1). */
export class BenchError extends Error {
  constructor(
    message: string,
    readonly exitCode: 1 | 2 = 2,
    options?: ErrorOptions,
  ) {
    super(message, options);
  }
}

export const Payloads = z.strictObject({
  version: z.literal(1),
  entries: z.record(z.string(), z.strictObject({ stdin: z.record(z.string(), z.unknown()) })),
});
export type Payloads = z.infer<typeof Payloads>;

export const Budgets = z.strictObject({
  version: z.literal(1),
  percentile: z.literal(50),
  entries: z.record(
    z.string(),
    z.strictObject({ maxRssMiB: z.number().positive(), cpuMs: z.number().positive() }),
  ),
});
export type Budgets = z.infer<typeof Budgets>;

/** One `cargo xtask measure` report. */
export const MeasureReport = z.strictObject({
  version: z.literal(1),
  command: z.array(z.string()),
  exitCode: z.number().int().nullable(),
  signal: z.number().int().nullable(),
  wallUs: z.number().int().nonnegative(),
  userUs: z.number().int().nonnegative(),
  sysUs: z.number().int().nonnegative(),
  maxRssBytes: z.number().int().nonnegative(),
});
export type MeasureReport = z.infer<typeof MeasureReport>;

const Spread = z.strictObject({ p50: z.number().nonnegative(), p90: z.number().nonnegative() });

export const EntryResult = z.strictObject({
  entry: z.string().regex(/^[a-z0-9-]+\/[a-z0-9-]+$/),
  event: z.string(),
  implementation: z.enum(["bun", "rust"]),
  maxRssBytes: Spread,
  cpuUs: Spread,
  wallUs: Spread,
});
export type EntryResult = z.infer<typeof EntryResult>;

export const HookResult = z.strictObject({
  schema: z.literal("toolu.hook-resources/v1"),
  provenance: z.strictObject({
    date: z.string().regex(/^\d{4}-\d{2}-\d{2}$/),
    commit: z.string(),
    platform: z.string(),
    arch: z.string(),
    cpu: z.string(),
    bun: z.string(),
    runner: z.enum(["github-actions", "local"]),
    runs: z.number().int().positive(),
    warmup: z.number().int().nonnegative(),
    elapsedMs: z.number().nonnegative(),
    floor: z.strictObject({
      maxRssBytes: z.number().nonnegative(),
      cpuUs: z.number().nonnegative(),
      wallUs: z.number().nonnegative(),
    }),
  }),
  entries: z.array(EntryResult),
});
export type HookResult = z.infer<typeof HookResult>;

/** Parse `file` as JSON against `schema`; any failure names the file. */
export function loadJson<T>(file: string, schema: z.ZodType<T>): T {
  let doc: unknown;
  try {
    doc = JSON.parse(readFileSync(file, "utf8"));
  } catch (error) {
    throw new BenchError(`${file} is not readable JSON`, 2, { cause: error });
  }
  const parsed = schema.safeParse(doc);
  if (!parsed.success) {
    throw new BenchError(`${file} is malformed: ${z.prettifyError(parsed.error)}`);
  }
  return parsed.data;
}
