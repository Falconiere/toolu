/**
 * What the gate-reach checks read, parsed with Zod. Every config goes through
 * `readJson`, so a missing file, invalid JSON or a wrong shape is a GateFatal
 * (exit 3) naming the file, never an empty rule set.
 */
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";

/** Misconfiguration: the gate cannot tell what it governs, so it must not pass. */
export class GateFatal extends Error {}

export function fatal(message: string): never {
  throw new GateFatal(message);
}

const ToolSchema = z.enum(["typecheck", "format", "oxlint", "jscpd", "knip"]);
export type Tool = z.infer<typeof ToolSchema>;

const Strings = z.array(z.string());
const Rules = z.record(z.string(), z.unknown());

/** tooling/gate-reach.json: trees outside the file universe, and declared gaps per tool. */
export const ReachConfigSchema = z.strictObject({
  version: z.literal(1),
  exclude: Strings,
  allowances: z.array(
    z.strictObject({ tool: ToolSchema, glob: z.string().min(1), why: z.string().min(1) }),
  ),
});

export const TsConfigSchema = z.looseObject({ include: Strings, exclude: Strings.default([]) });
export const PackageJsonSchema = z.looseObject({ scripts: z.record(z.string(), z.string()) });
export const JscpdSchema = z.looseObject({ path: Strings, ignore: Strings.default([]) });
export const KnipSchema = z.looseObject({
  workspaces: z.record(
    z.string(),
    z.looseObject({ project: Strings.default([]), ignore: Strings.default([]) }),
  ),
});
export const OxlintSchema = z.looseObject({
  extends: Strings.default([]),
  ignorePatterns: Strings.default([]),
  rules: Rules.default({}),
  overrides: z.array(z.looseObject({ files: Strings, rules: Rules.default({}) })).default([]),
});
export const WorkspaceSchema = z.looseObject({ packages: Strings });
export const PackageGuardrailsSchema = z.looseObject({ ownedByLinter: Strings.default([]) });

/** `rel` under `root`, parsed by `schema`; anything short of that is fatal. */
export function readJson<T>(root: string, rel: string, schema: z.ZodType<T>): T {
  let raw: unknown;
  try {
    raw = JSON.parse(readFileSync(join(root, rel), "utf8"));
  } catch (err: unknown) {
    return fatal(
      `${rel}: unreadable or not JSON (${err instanceof Error ? err.message : String(err)})`,
    );
  }
  const parsed = schema.safeParse(raw);
  if (!parsed.success) fatal(`${rel}: ${z.prettifyError(parsed.error).replaceAll("\n", "; ")}`);
  return parsed.data;
}
