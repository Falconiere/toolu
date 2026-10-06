/** Declarative quality-hook cases and their bounded real-sandbox environment setup. */
import { chmodSync, lstatSync, mkdirSync, readdirSync, symlinkSync, writeFileSync } from "node:fs";
import { delimiter, join } from "node:path";
import { z } from "zod";
import { ActionSchema, readCaseFile } from "./json-cases.ts";
import type { Sandbox } from "./sandbox.ts";
import type { EnvPatch } from "./spawn.ts";

const PatchFileSchema = z.strictObject({
  op: z.enum(["add", "update", "delete"]),
  path: z.string(),
  lines: z.array(z.string()).optional(),
});
const StepSchema = z.strictObject({
  write: z.record(z.string(), z.string()).optional(),
  remove: z.array(z.string()).optional(),
  tool: z.enum(["Write", "Edit", "MultiEdit", "Bash", "Delete"]).optional(),
  file: z.string().optional(),
  relative: z.boolean().optional(),
  input: z.record(z.string(), z.json()).optional(),
  patch: z.array(PatchFileSchema).optional(),
  rawPatch: z.string().optional(),
  env: z.record(z.string(), z.string()).optional(),
});
const EnvSchema = z.discriminatedUnion("kind", [
  z.strictObject({ kind: z.literal("pathWithout"), without: z.array(z.string()).min(1) }),
  z.strictObject({ kind: z.literal("stubAstGrep"), body: z.string() }),
  z.strictObject({
    kind: z.literal("projectSettings"),
    files: z.record(z.string().regex(/^[a-zA-Z0-9][a-zA-Z0-9._-]*$/), z.string()),
  }),
]);
const CaseSchema = z.strictObject({
  name: z.string().min(1),
  from: z.array(z.string()).optional(),
  hosts: z.array(z.enum(["claude", "codex"])).optional(),
  project: z.record(z.string(), z.string()),
  commit: z.record(z.string(), z.string()).optional(),
  config: z.record(z.string(), z.json()).optional(),
  register: z.array(z.string()).optional(),
  env: EnvSchema.optional(),
  setup: z.array(ActionSchema).optional(),
  steps: z.array(StepSchema).min(1),
  expect: z.enum(["block", "advisory", "silent", "exit2"]),
  contains: z.array(z.string()).optional(),
  absent: z.array(z.string()).optional(),
  unordered: z.boolean().optional(),
  deviation: z.strictObject({ contains: z.array(z.string()).min(1) }).optional(),
});

export type QualityCase = z.infer<typeof CaseSchema>;
export type QualityStep = z.infer<typeof StepSchema>;
export type QualityEnv = z.infer<typeof EnvSchema>;
/** Inline edge tests can still prepare a sandbox directly; the golden tables use JSON. */
export type QualityCaseInput = Omit<QualityCase, "setup" | "env"> & {
  setup?: QualityCase["setup"] | ((sb: Sandbox) => void) | undefined;
  env?: QualityEnv | ((sb: Sandbox) => EnvPatch) | undefined;
};

/** Load the committed records, rejecting malformed fields and duplicate names. */
export function readQualityCases(path: string): QualityCase[] {
  return readCaseFile(path).map((item) => CaseSchema.parse(item));
}

function present(path: string): boolean {
  try {
    lstatSync(path);
    return true;
  } catch {
    return false;
  }
}

function pathWithout(sb: Sandbox, without: readonly string[]): EnvPatch {
  const bin = join(sb.root, `path-without-${without.join("-")}`);
  mkdirSync(bin, { recursive: true });
  for (const dir of (process.env.PATH ?? "").split(delimiter)) {
    if (dir === "") continue;
    let names: string[];
    try {
      names = readdirSync(dir);
    } catch {
      continue;
    }
    for (const name of names) {
      const link = join(bin, name);
      if (without.includes(name) || present(link)) continue;
      symlinkSync(join(dir, name), link);
    }
  }
  return { PATH: bin };
}

/** Materialize the case's small environment variant for each tool step. */
export function qualityCaseEnv(sb: Sandbox, env: QualityEnv | undefined): EnvPatch {
  if (env === undefined) return {};
  if (env.kind === "pathWithout") return pathWithout(sb, env.without);
  if (env.kind === "projectSettings") {
    for (const [file, body] of Object.entries(env.files)) sb.write(`settings/${file}`, body);
    return { TOOLU_SETTINGS_DIR: sb.path("settings") };
  }
  const bin = join(sb.root, "stub-bin");
  mkdirSync(bin, { recursive: true });
  const path = join(bin, "ast-grep");
  writeFileSync(path, `#!/bin/sh\n${env.body}\n`);
  chmodSync(path, 0o755);
  return { PATH: `${bin}${delimiter}${process.env.PATH ?? ""}` };
}
