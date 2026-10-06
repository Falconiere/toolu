/**
 * Every payload the hook launcher (#250) emits must pass Codex's own output
 * schemas, vendored verbatim under fixtures/codex-hook-schemas/. The
 * payloads are produced by running the generated command for real with Bun
 * absent (advisory branch) and by the diagnostic helper the SessionStart bundle
 * prints (found branch). The enforcing branch writes nothing to stdout.
 *
 * The validator covers exactly the draft-07 keywords those schemas use and
 * throws on any other, so a schema refresh cannot silently weaken the check.
 */
import { afterAll, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { launcherCommand, runtimeDiagnostic } from "@toolu/core/launcher";

const SCHEMAS = resolve(import.meta.dir, "../../../fixtures/codex-hook-schemas");
const ANNOTATIONS = new Set(["$schema", "default", "description", "title", "definitions"]);
const home = mkdtempSync(join(tmpdir(), "launcher-schemas-"));

afterAll(() => rmSync(home, { recursive: true, force: true }));

type Schema = Record<string, unknown>;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function typeOf(value: unknown): string {
  if (value === null) return "null";
  if (Array.isArray(value)) return "array";
  if (typeof value === "number") return Number.isInteger(value) ? "integer" : "number";
  return typeof value;
}

function typeMatches(expected: unknown, value: unknown): boolean {
  const types = Array.isArray(expected) ? expected : [expected];
  const actual = typeOf(value);
  return types.some((t) => t === actual || (t === "number" && actual === "integer"));
}

function resolveRef(root: Schema, ref: unknown): Schema {
  const name = typeof ref === "string" ? ref.replace("#/definitions/", "") : "";
  const definitions = root.definitions;
  const target = isRecord(definitions) ? definitions[name] : undefined;
  if (!isRecord(target)) throw new Error(`unresolvable $ref ${String(ref)}`);
  return target;
}

function validateObject(root: Schema, schema: Schema, value: unknown, at: string): string[] {
  if (!isRecord(value)) return [];
  const properties = isRecord(schema.properties) ? schema.properties : {};
  const errors: string[] = [];
  for (const key of Array.isArray(schema.required) ? schema.required : []) {
    if (typeof key === "string" && !(key in value)) errors.push(`${at}.${key}: required`);
  }
  for (const [key, child] of Object.entries(value)) {
    const sub = properties[key];
    if (isRecord(sub)) errors.push(...validate(root, sub, child, `${at}.${key}`));
    else if (schema.additionalProperties === false) errors.push(`${at}.${key}: not allowed`);
  }
  return errors;
}

/** Errors for `value` against `schema`; empty when valid. */
function validate(root: Schema, schema: Schema, value: unknown, at = "$"): string[] {
  const known = new Set(["$ref", "allOf", "additionalProperties", "const", "enum", "properties"]);
  for (const key of ["required", "type"]) known.add(key);
  const unknown = Object.keys(schema).filter((k) => !known.has(k) && !ANNOTATIONS.has(k));
  if (unknown.length > 0) throw new Error(`${at}: unsupported keywords ${unknown.join(", ")}`);
  if (schema.$ref !== undefined) return validate(root, resolveRef(root, schema.$ref), value, at);
  const errors: string[] = [];
  if (schema.type !== undefined && !typeMatches(schema.type, value)) {
    errors.push(`${at}: expected ${JSON.stringify(schema.type)}, got ${typeOf(value)}`);
  }
  if (Array.isArray(schema.enum) && !schema.enum.includes(value)) errors.push(`${at}: not in enum`);
  if ("const" in schema && schema.const !== value) errors.push(`${at}: not const`);
  for (const part of Array.isArray(schema.allOf) ? schema.allOf : []) {
    if (isRecord(part)) errors.push(...validate(root, part, value, at));
  }
  return [...errors, ...validateObject(root, schema, value, at)];
}

function codexErrors(event: string, payload: unknown): string[] {
  const text = readFileSync(join(SCHEMAS, `${event}.command.output.schema.json`), "utf8");
  const root: unknown = JSON.parse(text);
  if (!isRecord(root)) throw new Error(`${event}: schema is not an object`);
  return validate(root, root, payload);
}

/** The advisory the generated command really prints for `event` with Bun absent. */
function advisoryStdout(event: string): unknown {
  const command = launcherCommand({ plugin: "toolu", event, entry: "session-start" });
  const result = spawnSync("sh", ["-c", command], {
    env: { PATH: "/usr/bin:/bin", HOME: home, CLAUDE_PLUGIN_ROOT: home },
    encoding: "utf8",
  });
  expect(result.status).toBe(0);
  return JSON.parse(result.stdout);
}

const ADVISORY = [
  ["SessionStart", "session-start"],
  ["UserPromptSubmit", "user-prompt-submit"],
  ["PostToolUse", "post-tool-use"],
] as const;

for (const [event, schema] of ADVISORY) {
  test(`the missing-runtime ${event} advisory passes the Codex ${schema} output schema`, () => {
    expect(codexErrors(schema, advisoryStdout(event))).toEqual([]);
  });
}

test("the found-runtime diagnostic passes the Codex session-start output schema", () => {
  expect(codexErrors("session-start", runtimeDiagnostic(process.execPath, Bun.version))).toEqual(
    [],
  );
});

test("the enforcing branch prints no stdout payload", () => {
  const command = launcherCommand({ plugin: "toolu", event: "PreToolUse", entry: "session-start" });
  const result = spawnSync("sh", ["-c", command], {
    env: { PATH: "/usr/bin:/bin", HOME: home, CLAUDE_PLUGIN_ROOT: home },
    encoding: "utf8",
  });
  expect(result.status).toBe(2);
  expect(result.stdout).toBe("");
});

test("a systemMessage payload is also valid on the enforcing events' schemas", () => {
  for (const schema of ["pre-tool-use", "permission-request"]) {
    expect(codexErrors(schema, runtimeDiagnostic("/x", "1.4.2"))).toEqual([]);
  }
});

test("negative controls: the validator rejects an extra key and a wrong type", () => {
  expect(codexErrors("session-start", { systemMessage: "x", bogus: true })).toEqual([
    "$.bogus: not allowed",
  ]);
  expect(codexErrors("session-start", { systemMessage: 1 })).toEqual([
    '$.systemMessage: expected "string", got integer',
  ]);
  expect(
    codexErrors("pre-tool-use", { hookSpecificOutput: { hookEventName: "PreToolUse", x: 1 } }),
  ).not.toEqual([]);
});
