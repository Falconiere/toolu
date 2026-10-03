/** Strict persistence boundary for per-issue launch records. */

import { readFileSync } from "node:fs";

const LAUNCH_STAGES = new Set([
  "uncertain",
  "replacing",
  "starting",
  "cleaning",
  "cleanup-incomplete",
  "running",
  "awaiting_merge",
  "merged",
  "abandoned",
]);

const IDENTITY_FIELDS = [
  "lease_token",
  "resource_root",
  "pane_id",
  "worktree",
  "session_id",
] as const;

function invalid(path: string, detail: string): Error {
  return new Error(`invalid launch record ${path}: ${detail}`);
}

function isMissing(error: unknown): boolean {
  return (
    error instanceof Error && "code" in error && (error as NodeJS.ErrnoException).code === "ENOENT"
  );
}

function validateIdentity(path: string, record: Record<string, unknown>): void {
  for (const field of IDENTITY_FIELDS) {
    if (!Object.hasOwn(record, field)) continue;
    const value = record[field];
    if (field === "session_id" && value === null) {
      const cleanupOrTerminal = ["cleaning", "cleanup-incomplete", "merged", "abandoned"].includes(
        String(record.stage),
      );
      const awaitingCapture =
        record.prompt_state !== "acknowledged" &&
        ["starting", "uncertain", "replacing"].includes(String(record.stage));
      if (!cleanupOrTerminal && !awaitingCapture) {
        throw invalid(path, "session_id may be null only while awaiting capture");
      }
      continue;
    }
    if (typeof value !== "string" || value.trim().length === 0)
      throw invalid(path, `${field} must be a non-empty string`);
  }
  const finishedOrCleaning = ["cleaning", "cleanup-incomplete", "merged", "abandoned"].includes(
    String(record.stage),
  );
  const managedRunning =
    typeof record.lease_token === "string" &&
    !finishedOrCleaning &&
    (record.stage === "running" || record.prompt_state === "acknowledged");
  if (managedRunning && typeof record.session_id !== "string")
    throw invalid(path, "a managed running launch requires session_id");
}

/** Read and validate one durable issue launch record. Missing records are new attempts. */
export function loadLaunchRecord(path: string): Record<string, unknown> {
  let source: string;
  try {
    source = readFileSync(path, "utf8");
  } catch (error) {
    if (isMissing(error)) return {};
    throw error;
  }

  let value: unknown;
  try {
    value = JSON.parse(source);
  } catch {
    throw invalid(path, "malformed JSON");
  }
  if (value === null || typeof value !== "object" || Array.isArray(value))
    throw invalid(path, "expected a JSON object");

  const record = value as Record<string, unknown>;
  if (
    Object.hasOwn(record, "stage") &&
    (typeof record.stage !== "string" || !LAUNCH_STAGES.has(record.stage))
  ) {
    throw invalid(path, `unknown stage ${JSON.stringify(record.stage)}`);
  }
  if (
    Object.hasOwn(record, "launches") &&
    (typeof record.launches !== "number" ||
      !Number.isSafeInteger(record.launches) ||
      record.launches < 0)
  ) {
    throw invalid(path, "launches must be a nonnegative integer");
  }
  if (
    Object.hasOwn(record, "session_started_after") &&
    (typeof record.session_started_after !== "number" ||
      !Number.isFinite(record.session_started_after) ||
      record.session_started_after < 0)
  ) {
    throw invalid(path, "session_started_after must be a finite epoch");
  }
  validateIdentity(path, record);
  return record;
}
