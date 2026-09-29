/**
 * Typed reads over a parsed config document. Each reader names the file and
 * key on a type error and fails closed (exit 3); an absent optional key reads
 * to its default, exactly like the bash jq `// []` fallbacks.
 */
import { fatal } from "./report.ts";

export type Doc = ReadonlyMap<string, unknown>;

/** A JSON scalar the way `jq -r` prints it; objects and arrays in compact JSON. */
export function jqText(value: unknown): string {
  if (value === null || value === undefined) return "null";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return JSON.stringify(value);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function typeError(file: string, key: string, want: string): never {
  fatal(`${file}: "${key}" must be ${want} — fix the value; the schema documents every key`);
}

/** An object-valued key, wrapped in a Doc; absent or null reads empty. */
export function objectOf(file: string, doc: Doc, key: string): Doc {
  const value = doc.get(key);
  if (value === undefined || value === null) return new Map();
  if (!isRecord(value)) typeError(file, key, "an object");
  return new Map(Object.entries(value));
}

export function stringField(file: string, doc: Doc, key: string): string {
  const value = doc.get(key);
  if (typeof value !== "string") typeError(file, key, "a string");
  return value;
}

export function numberField(file: string, doc: Doc, key: string): number {
  const value = doc.get(key);
  if (typeof value !== "number") typeError(file, key, "a number");
  return value;
}

/** Array of strings; required. */
export function strings(file: string, doc: Doc, key: string): string[] {
  const value = doc.get(key);
  if (!Array.isArray(value) || !value.every((item) => typeof item === "string")) {
    typeError(file, key, "an array of strings");
  }
  return value.map(String);
}

/** Array of strings; absent or null reads as []. */
export function optionalStrings(file: string, doc: Doc, key: string): string[] {
  const value = doc.get(key);
  return value === undefined || value === null ? [] : strings(file, doc, key);
}

/** Array of objects mapped through `read`; absent or null reads as []. */
export function optionalArray<T>(
  file: string,
  doc: Doc,
  key: string,
  read: (entry: Doc) => T,
): T[] {
  const value = doc.get(key);
  if (value === undefined || value === null) return [];
  if (!Array.isArray(value)) typeError(file, key, "an array");
  return value.map((entry: unknown) => {
    if (!isRecord(entry)) typeError(file, key, "an array of objects");
    return read(new Map(Object.entries(entry)));
  });
}

/** An object whose values all satisfy `read`, in document order. */
export function entriesOf<T>(
  file: string,
  doc: Doc,
  key: string,
  read: (value: unknown) => T | null,
): Array<[string, T]> {
  const out: Array<[string, T]> = [];
  for (const [name, value] of objectOf(file, doc, key)) {
    const parsed = read(value);
    if (parsed === null) typeError(file, `${key}.${name}`, "a valid value");
    out.push([name, parsed]);
  }
  return out;
}

export function stringList(value: unknown): string[] | null {
  return Array.isArray(value) && value.every((item) => typeof item === "string")
    ? value.map(String)
    : null;
}
