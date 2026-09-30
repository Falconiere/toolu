/** JSON object access shared by the payload reader, the collector and setup. */
import { readFileSync } from "node:fs";

/** A JSON object's members, or undefined for anything else (null, array, scalar). */
export function asObject(value: unknown): Record<string, unknown> | undefined {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? Object.fromEntries(Object.entries(value))
    : undefined;
}

/** A file's JSON object, or undefined when it is absent, unreadable, unparseable or not an object. */
export function readObject(path: string): Record<string, unknown> | undefined {
  try {
    return asObject(JSON.parse(readFileSync(path, "utf8")));
  } catch {
    return undefined;
  }
}
