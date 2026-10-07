/**
 * The shared dispatch fixture (#418): one case of `fixtures/dispatch/cases.json`
 * set up in a sandbox and run through the TypeScript dispatcher. The Rust engine
 * (`crates/core/engine/tests/dispatch_fixture.rs`) runs the same cases.
 */
import { chmodSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";
import { DecisionSchema } from "../../decision/decision.ts";
import {
  dispatchPostTool,
  dispatchPreTool,
  type ModuleResult,
  type ToolModule,
} from "../dispatch.ts";

export const FIXTURE = resolve(import.meta.dir, "../../../../../fixtures/dispatch/cases.json");

const BuiltinSchema = z.union([
  z.strictObject({ name: z.string().min(1), decision: DecisionSchema }),
  z.strictObject({ name: z.string().min(1), throws: z.string() }),
]);
const ModuleSchema = z.union([
  z.strictObject({ file: z.string().min(1), sh: z.string() }),
  z.strictObject({ file: z.string().min(1), js: z.string() }),
]);
const ExpectSchema = z.strictObject({
  stdout: z.string(),
  stderr: z.string(),
  exitCode: z.number().int(),
});

export const CaseSchema = z.strictObject({
  name: z.string().min(1),
  phase: z.enum(["pre", "post"]),
  host: z.enum(["claude", "codex"]),
  stdin: z.union([z.string(), z.record(z.string(), z.unknown())]),
  config: z.record(z.string(), z.unknown()).optional(),
  builtins: z.array(BuiltinSchema).optional(),
  registry: z.array(ModuleSchema).optional(),
  installed: z.array(z.string()).optional(),
  installedRaw: z.string().optional(),
  continuePostBlocks: z.boolean().optional(),
  expect: ExpectSchema.optional(),
});
export type DispatchCase = z.infer<typeof CaseSchema>;

const TOKENS = ["PROJECT", "HOME", "CODEX", "CONFIG", "LIB"] as const;
export type Paths = Record<(typeof TOKENS)[number], string>;

/** `$PROJECT`, `$HOME`, `$CODEX`, `$CONFIG` and `$LIB`, never a longer name such as `$PROJECT_ROOT`. */
const TOKEN = /\$(PROJECT|HOME|CODEX|CONFIG|LIB)(?![A-Za-z0-9_])/g;

export function expand(text: string, paths: Paths): string {
  return text.replace(TOKEN, (_match, key: keyof Paths) => paths[key]);
}

function expandValue(value: unknown, paths: Paths): unknown {
  if (typeof value === "string") return expand(value, paths);
  if (Array.isArray(value)) return value.map((item) => expandValue(item, paths));
  if (typeof value === "object" && value !== null) {
    return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, expandValue(v, paths)]));
  }
  return value;
}

/** Sandbox paths back to their tokens, the longest first. */
export function tokenize(text: string, paths: Paths): string {
  const order = ["CONFIG", "LIB", "CODEX", "PROJECT", "HOME"] as const;
  return order.reduce((out, key) => out.replaceAll(paths[key], `$${key}`), text);
}

function pathsOf(sb: Sandbox, host: DispatchCase["host"]): Paths {
  const config = host === "codex" ? sb.codexHome : join(sb.home, ".claude");
  const lib = join(sb.root, "plugin", "hooks", "lib");
  return { PROJECT: sb.project, HOME: sb.home, CODEX: sb.codexHome, CONFIG: config, LIB: lib };
}

function writeAt(path: string, body: string): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, body);
}

function setup(sb: Sandbox, c: DispatchCase, paths: Paths): void {
  const dir = join(paths.CONFIG, "toolu", c.phase === "pre" ? "pre-tools.d" : "post-tools.d");
  for (const module of c.registry ?? []) {
    const path = join(dir, module.file);
    if ("sh" in module) {
      writeAt(path, `#!/usr/bin/env bash\n${expand(module.sh, paths)}\n`);
      chmodSync(path, 0o755);
    } else writeAt(path, expand(module.js, paths));
  }
  if (c.config !== undefined) {
    const dirname = c.host === "codex" ? ".codex" : ".claude";
    writeAt(join(sb.project, dirname, "toolu.config.json"), JSON.stringify(c.config));
  }
  if (c.host === "codex" && c.installed !== undefined) {
    const snapshot = { version: 1, status: "ready", plugins: c.installed };
    writeAt(join(paths.CONFIG, "toolu", "codex-plugins.json"), JSON.stringify(snapshot));
  }
  const record = join(sb.home, ".claude", "plugins", "installed_plugins.json");
  if (c.host === "claude" && c.installed !== undefined) {
    const plugins = Object.fromEntries(c.installed.map((spec) => [spec, [{ scope: "user" }]]));
    writeAt(record, JSON.stringify({ version: 2, plugins }));
  }
  if (c.installedRaw !== undefined) writeAt(record, c.installedRaw);
}

function builtinsOf(c: DispatchCase): ToolModule[] {
  return (c.builtins ?? []).map((b) => ({
    kind: "native",
    name: b.name,
    run: () => ("throws" in b ? Promise.reject(new Error(b.throws)) : Promise.resolve(b.decision)),
  }));
}

function envOf(sb: Sandbox, host: DispatchCase["host"]): Record<string, string> {
  const base = { PATH: process.env.PATH ?? "/usr/bin:/bin", HOME: sb.home };
  return host === "codex"
    ? { ...base, PLUGIN_ROOT: sb.root, CODEX_HOME: sb.codexHome }
    : { ...base, CLAUDE_PROJECT_DIR: sb.project };
}

/** Run `c` in a fresh sandbox; the result's paths are the sandbox's own. */
export async function runCase(c: DispatchCase): Promise<{ result: ModuleResult; paths: Paths }> {
  using sb = createSandbox({ git: true });
  const paths = pathsOf(sb, c.host);
  setup(sb, c, paths);
  const stdin =
    typeof c.stdin === "string"
      ? expand(c.stdin, paths)
      : JSON.stringify(expandValue(c.stdin, paths));
  const options = {
    builtins: builtinsOf(c),
    libDir: paths.LIB,
    env: envOf(sb, c.host),
    cwd: sb.project,
    ...(c.continuePostBlocks === true ? { continuePostBlocks: true } : {}),
  };
  const result =
    c.phase === "pre"
      ? await dispatchPreTool(stdin, options)
      : await dispatchPostTool(stdin, options);
  return { result, paths };
}
