/** Shared JSON case loading and sandbox setup for the TypeScript/Rust parity fixtures. */
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, isAbsolute, relative, resolve } from "node:path";
import { z } from "zod";
import type { HostName, Sandbox } from "./sandbox.ts";

const HostSchema = z.enum(["claude", "codex", "cursor", "opencode"]);
const ScopeSchema = z.enum(["project", "user"]);
const ActionSchema = z.discriminatedUnion("op", [
  z.strictObject({ op: z.literal("write"), path: z.string(), body: z.string() }),
  z.strictObject({ op: z.literal("remove"), path: z.string() }),
  z.strictObject({ op: z.literal("git"), args: z.array(z.string()).min(1) }),
  z.strictObject({
    op: z.literal("config"),
    host: HostSchema,
    scope: ScopeSchema,
    body: z.record(z.string(), z.json()),
  }),
]);
const CaseSchema = z.looseObject({
  name: z.string().min(1),
  setup: z.array(ActionSchema).optional(),
});
const CaseFileSchema = z.strictObject({
  version: z.literal(1),
  cases: z.array(CaseSchema),
});

export type SetupAction = z.infer<typeof ActionSchema>;
export type JsonCase = z.infer<typeof CaseSchema>;

/** Load a committed case file, rejecting malformed envelopes and duplicate names. */
export function readCaseFile(path: string): JsonCase[] {
  const parsed = CaseFileSchema.parse(JSON.parse(readFileSync(path, "utf8")));
  const names = new Set<string>();
  for (const item of parsed.cases) {
    if (names.has(item.name)) throw new Error(`${path}: duplicate case name: ${item.name}`);
    names.add(item.name);
  }
  return parsed.cases;
}

/** Golden captures must have exactly one entry for each named input case. */
export function assertCaptureNames(cases: readonly JsonCase[], captures: unknown): void {
  const parsed = z.record(z.string(), z.unknown()).parse(captures);
  const actual = Object.keys(parsed).toSorted();
  const expected = cases.map((item) => item.name).toSorted();
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error("capture names differ from case names");
  }
}

function inside(base: string, rel: string): string {
  const abs = resolve(base, rel);
  const back = relative(base, abs);
  if (back.startsWith("..") || isAbsolute(back)) {
    throw new Error(`fixture path escapes sandbox: ${rel}`);
  }
  return abs;
}

/** Expand only a path field; never interpolate source files, commands, or golden text. */
export function resolveFixturePath(sb: Sandbox, path: string, host: HostName = "claude"): string {
  if (path === "$PROJECT") return sb.project;
  if (path.startsWith("$PROJECT/")) return inside(sb.project, path.slice(9));
  if (path === "$ROOT") return sb.root;
  if (path.startsWith("$ROOT/")) return inside(sb.root, path.slice(6));
  if (path === "$HOME") return sb.home;
  if (path.startsWith("$HOME/")) return inside(sb.home, path.slice(6));
  const state = sb.configDir(host, "project");
  if (path === "$HOST_STATE") return state;
  if (path.startsWith("$HOST_STATE/")) return inside(state, path.slice(12));
  if (path.startsWith("$")) throw new Error(`unknown path token: ${path}`);
  if (isAbsolute(path)) throw new Error(`fixture path escapes sandbox: ${path}`);
  return sb.path(path);
}

/** Materialize explicitly tagged dynamic values, leaving ordinary command text unchanged. */
export function materializeCaseValue(
  sb: Sandbox,
  value: unknown,
  host: HostName = "claude",
): unknown {
  if (Array.isArray(value)) return value.map((item) => materializeCaseValue(sb, item, host));
  if (value === null || typeof value !== "object") return value;
  const entries = Object.entries(value);
  if (entries.length === 1 && entries[0]?.[0] === "$path") {
    const path: unknown = entries[0][1];
    if (typeof path !== "string") throw new Error("$path must be a string");
    return resolveFixturePath(sb, path, host);
  }
  if (entries.length === 1 && entries[0]?.[0] === "$template") {
    const template: unknown = entries[0][1];
    if (typeof template !== "string") throw new Error("$template must be a string");
    const roots: Record<string, string> = {
      $PROJECT: sb.project,
      $ROOT: sb.root,
      $HOME: sb.home,
      $HOST_STATE: sb.configDir(host, "project"),
    };
    return template.replace(/\$[A-Z][A-Z_]*(?=\/|\b)/g, (token) => {
      const root = roots[token];
      if (root === undefined) throw new Error(`unknown path token: ${token}`);
      return root;
    });
  }
  return Object.fromEntries(
    entries.map(([key, item]) => [key, materializeCaseValue(sb, item, host)]),
  );
}

function write(path: string, body: string): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, body);
}

/** Execute the small setup vocabulary against a disposable real sandbox. */
export function applyCaseSetup(sb: Sandbox, actions: unknown, host: HostName = "claude"): void {
  const parsed = z.array(ActionSchema).parse(actions);
  for (const action of parsed) {
    switch (action.op) {
      case "write":
        write(resolveFixturePath(sb, action.path, host), action.body);
        break;
      case "remove":
        rmSync(resolveFixturePath(sb, action.path, host), { recursive: true, force: true });
        break;
      case "git":
        sb.git(...action.args);
        break;
      case "config":
        sb.writeConfig(action.host, action.scope, action.body);
        break;
    }
  }
}
