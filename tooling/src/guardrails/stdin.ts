/** Hook payload reading with the bash runner's forgiving jq semantics. */
import { readFileSync } from "node:fs";

/** All of stdin; empty when there is none to read. */
export function readStdin(): string {
  try {
    return readFileSync(0, "utf8");
  } catch {
    return "";
  }
}

function parse(payload: string): unknown {
  try {
    return JSON.parse(payload);
  } catch {
    return null;
  }
}

function field(value: unknown, key: string): unknown {
  return typeof value === "object" && value !== null ? Reflect.get(value, key) : undefined;
}

/** `jq -r '.tool_input.file_path // empty'`: "" when absent, null, false or unparseable. */
export function editedPath(payload: string): string {
  const value = field(field(parse(payload), "tool_input"), "file_path");
  if (typeof value === "string" || typeof value === "number" || value === true) return String(value);
  return "";
}

/** `jq -r '.stop_hook_active // false'` printed "true". */
export function stopHookActive(payload: string): boolean {
  const value = field(parse(payload), "stop_hook_active");
  return value === true || value === "true";
}
