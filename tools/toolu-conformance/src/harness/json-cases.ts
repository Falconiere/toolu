/** Shared JSON case loading and sandbox setup for the TypeScript/Rust parity fixtures. */
import { chmodSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { dirname, isAbsolute, relative, resolve } from "node:path";
import { pushWaiverPend } from "@toolu/core/ledger";
import { diffSha } from "@toolu/core/state";
import { z } from "zod";
import type { ToolFixture } from "./fixtures.ts";
import type { HostName, Sandbox } from "./sandbox.ts";

const HostSchema = z.enum(["claude", "codex", "cursor", "opencode"]);
const ScopeSchema = z.enum(["project", "user"]);
const REPO_ROOT = resolve(import.meta.dir, "../../../..");
const TaggedPathSchema = z.strictObject({ $path: z.string() });
const TaggedTemplateSchema = z.strictObject({ $template: z.string() });
export const ActionSchema = z.discriminatedUnion("op", [
  z.strictObject({
    op: z.literal("write"),
    path: z.string(),
    body: z.union([z.string(), TaggedTemplateSchema]),
  }),
  z.strictObject({ op: z.literal("remove"), path: z.string() }),
  z.strictObject({ op: z.literal("mkdir"), path: z.string() }),
  z.strictObject({
    op: z.literal("symlink"),
    path: z.string(),
    target: z.union([z.string(), TaggedPathSchema, TaggedTemplateSchema]),
  }),
  z.strictObject({
    op: z.literal("chmod"),
    path: z.string(),
    mode: z.string().regex(/^[0-7]{3}$/),
  }),
  z.strictObject({
    op: z.literal("git"),
    args: z.array(z.union([z.string(), TaggedPathSchema, TaggedTemplateSchema])).min(1),
  }),
  z.strictObject({
    op: z.literal("config"),
    host: HostSchema,
    scope: ScopeSchema,
    body: z.record(z.string(), z.json()),
  }),
  z.strictObject({
    op: z.literal("push-waiver-pend"),
    slug: z.string().min(1),
    base: z.string().min(1),
    reasonCode: z.string().min(1),
  }),
]);
export const HostSetupSchema = z.strictObject({
  claude: z.array(ActionSchema),
  codex: z.array(ActionSchema),
});
const CaseSchema = z.looseObject({
  name: z.string().min(1),
  setup: z.array(ActionSchema).optional(),
});
const CaseFileSchema = z.strictObject({
  version: z.literal(1),
  cases: z.array(CaseSchema),
});
const ToolFixtureSchema = z.strictObject({
  kind: z.literal("tool"),
  event: z.enum(["PreToolUse", "PostToolUse"]),
  toolName: z.string(),
  toolInput: z.record(z.string(), z.unknown()),
  toolResponse: z.unknown().optional(),
  mcp: z.strictObject({ server: z.string(), tool: z.string() }).optional(),
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

/** Validate a tool event after expanding its tagged sandbox paths. */
export function materializeToolFixture(sb: Sandbox, value: unknown, host: HostName): ToolFixture {
  const { mcp, ...base } = ToolFixtureSchema.parse(materializeCaseValue(sb, value, host));
  return mcp === undefined ? base : { ...base, mcp };
}

function write(path: string, body: string): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, body);
}

/** The checkout token is allowed only as a symlink target, never a write path. */
function symlinkTarget(sb: Sandbox, target: unknown, host: HostName): string {
  if (
    target !== null &&
    typeof target === "object" &&
    "$path" in target &&
    typeof target.$path === "string" &&
    target.$path.startsWith("$REPO/")
  ) {
    return inside(REPO_ROOT, target.$path.slice(6));
  }
  return z.string().parse(materializeCaseValue(sb, target, host));
}

/** Execute the small setup vocabulary against a disposable real sandbox. */
export function applyCaseSetup(sb: Sandbox, actions: unknown, host: HostName = "claude"): void {
  const parsed = z.array(ActionSchema).parse(actions);
  for (const action of parsed) {
    switch (action.op) {
      case "write":
        write(
          resolveFixturePath(sb, action.path, host),
          z.string().parse(materializeCaseValue(sb, action.body, host)),
        );
        break;
      case "remove":
        rmSync(resolveFixturePath(sb, action.path, host), { recursive: true, force: true });
        break;
      case "mkdir":
        mkdirSync(resolveFixturePath(sb, action.path, host), { recursive: true });
        break;
      case "symlink": {
        const path = resolveFixturePath(sb, action.path, host);
        mkdirSync(dirname(path), { recursive: true });
        symlinkSync(symlinkTarget(sb, action.target, host), path);
        break;
      }
      case "chmod":
        chmodSync(resolveFixturePath(sb, action.path, host), parseInt(action.mode, 8));
        break;
      case "git":
        sb.git(...action.args.map((arg) => z.string().parse(materializeCaseValue(sb, arg, host))));
        break;
      case "config":
        sb.writeConfig(action.host, action.scope, action.body);
        break;
      case "push-waiver-pend": {
        const env = { HOME: sb.home, PATH: process.env.PATH ?? "/usr/bin:/bin" };
        const sha = diffSha(sb.project, action.base, { env });
        if (sha === undefined) throw new Error("fixture could not compute push waiver diff SHA");
        if (
          !pushWaiverPend(sb.project, action.slug, sha, action.base, action.reasonCode, {
            env,
            host,
          })
        ) {
          throw new Error("fixture could not create pending push waiver");
        }
        break;
      }
    }
  }
}
