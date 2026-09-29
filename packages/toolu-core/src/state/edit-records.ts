/**
 * Edit-payload normalization (#255), a port of `edit-records.sh`. Edit,
 * Write, MultiEdit and Codex `apply_patch` payloads become one record per
 * affected path. A move yields a source record (`update` + `moved_to`) and a
 * destination record (`move` + `from`), so path-based gates inspect both
 * sides. A malformed payload yields no partial records; pre-tool callers
 * treat that as a fail-closed signal.
 */
import { isJsonObject } from "../config/config-load.ts";
import { toJqJson } from "./state-io.ts";
import type { EditRecord } from "./state-schema.ts";

export const EDIT_TOOLS = ["Edit", "Write", "MultiEdit", "apply_patch"] as const;

export type EditRecordsResult =
  | { kind: "records"; records: EditRecord[] }
  | { kind: "not-edit" }
  | { kind: "malformed" };

/** `toolu_is_edit_tool`. */
export function isEditTool(tool: string): boolean {
  return EDIT_TOOLS.some((name) => name === tool);
}

/** Non-empty, single-line, tab-free: `_toolu_patch_path_valid`. */
function pathValid(path: string): boolean {
  return path !== "" && !/[\n\r\t]/.test(path);
}

/** A string read through `$(jq -r ...)`: command substitution strips trailing newlines. */
function substituted(value: string): string {
  return value.replace(/\n+$/, "");
}

type ToolInput = Record<string, unknown> | null;

/** `.tool_input` as jq sees it: null when absent; undefined when jq would error on indexing. */
function toolInput(payload: unknown): ToolInput | undefined {
  if (payload === null) return null;
  if (!isJsonObject(payload)) return undefined;
  const input = payload.tool_input;
  if (input === undefined || input === null) return null;
  return isJsonObject(input) ? input : undefined;
}

/** jq `a // b`: the first value that is neither null nor false (absent reads as null). */
function truthy(value: unknown): boolean {
  return value !== undefined && value !== null && value !== false;
}

/** `.tool_input.file_path // .tool_input.path // .tool_input.target_file | strings`. */
function editPath(input: ToolInput): string | undefined {
  const value = [input?.file_path, input?.path].find(truthy) ?? input?.target_file;
  return typeof value === "string" ? substituted(value) : undefined;
}

type PatchState = {
  records: EditRecord[];
  pending: string;
  begun: boolean;
  ended: boolean;
  headers: number;
  invalid: boolean;
};

function flushPending(s: PatchState): void {
  if (s.pending !== "") s.records.push({ path: s.pending, operation: "update" });
  s.pending = "";
}

/** The path after a `*** <header>:` prefix: exactly one space, then a valid path. */
function headerPath(line: string, prefix: string): string | undefined {
  const raw = line.slice(prefix.length);
  if (!raw.startsWith(" ")) return undefined;
  const path = raw.slice(1);
  return pathValid(path) ? path : undefined;
}

function fileHeader(
  s: PatchState,
  line: string,
  prefix: string,
  operation: "add" | "delete",
): void {
  if (!s.begun) {
    s.invalid = true;
    return;
  }
  flushPending(s);
  const path = headerPath(line, prefix);
  if (path === undefined) {
    s.invalid = true;
    return;
  }
  s.records.push({ path, operation });
  s.headers += 1;
}

function updateHeader(s: PatchState, line: string): void {
  if (!s.begun) {
    s.invalid = true;
    return;
  }
  flushPending(s);
  const path = headerPath(line, "*** Update File:");
  if (path === undefined) {
    s.invalid = true;
    return;
  }
  s.pending = path;
  s.headers += 1;
}

function moveHeader(s: PatchState, line: string): void {
  const target = s.begun && s.pending !== "" ? headerPath(line, "*** Move to:") : undefined;
  if (target === undefined) {
    s.invalid = true;
    return;
  }
  s.records.push({ path: s.pending, operation: "update", moved_to: target });
  s.records.push({ path: target, operation: "move", from: s.pending });
  s.pending = "";
  s.headers += 1;
}

function patchLine(s: PatchState, line: string): void {
  if (s.ended) {
    if (line !== "") s.invalid = true;
  } else if (line === "*** Begin Patch") {
    if (s.begun) s.invalid = true;
    s.begun = true;
  } else if (line === "*** End Patch") {
    if (s.begun) {
      flushPending(s);
      s.ended = true;
    } else {
      s.invalid = true;
    }
  } else if (line.startsWith("*** Add File:")) {
    fileHeader(s, line, "*** Add File:", "add");
  } else if (line.startsWith("*** Update File:")) {
    updateHeader(s, line);
  } else if (line.startsWith("*** Delete File:")) {
    fileHeader(s, line, "*** Delete File:", "delete");
  } else if (line.startsWith("*** Move to:")) {
    moveHeader(s, line);
  } else if (line === "*** End of File") {
    // Optional EOF marker inside the current file operation; it names no path.
    if (!s.begun || s.headers === 0) s.invalid = true;
  } else if (line.startsWith("*** ")) {
    // An unknown control header may carry a path a newer grammar introduced: never skip it.
    s.invalid = true;
  }
}

/** `_toolu_apply_patch_records`: every affected path, or undefined when the patch is malformed. */
export function applyPatchRecords(patch: string): EditRecord[] | undefined {
  const s: PatchState = {
    records: [],
    pending: "",
    begun: false,
    ended: false,
    headers: 0,
    invalid: false,
  };
  // A here-string feeds `read` one line per "\n"; each line loses one trailing CR.
  for (const line of patch.split("\n")) {
    patchLine(s, line.replace(/\r$/, ""));
  }
  return s.invalid || !s.begun || !s.ended || s.headers === 0 ? undefined : s.records;
}

/** `toolu_normalize_edit_records INPUT TOOL`, over the parsed hook payload. */
export function normalizeEditRecords(payload: unknown, tool: string): EditRecordsResult {
  if (!isEditTool(tool)) return { kind: "not-edit" };
  const input = toolInput(payload);
  if (input === undefined) return { kind: "malformed" };
  if (tool === "apply_patch") {
    const command = input?.command;
    const records =
      typeof command === "string" ? applyPatchRecords(substituted(command)) : undefined;
    return records === undefined ? { kind: "malformed" } : { kind: "records", records };
  }
  const path = editPath(input);
  if (path === undefined || !pathValid(path)) return { kind: "malformed" };
  return { kind: "records", records: [{ path, operation: tool === "Write" ? "write" : "update" }] };
}

/** Records as the bash function prints them: one `jq -c` object per line. */
export function formatEditRecords(records: readonly EditRecord[]): string {
  return records.map((record) => `${toJqJson(record, false)}\n`).join("");
}
