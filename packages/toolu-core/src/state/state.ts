/**
 * `@toolu/core/state` (#255): gate state, stale-state sweeping, the
 * branch-diff hash, telemetry and edit-payload normalization. It ports
 * `gate-file.sh`, `state-sweeper.sh`, `diff-sha.sh`, `telemetry.sh` and
 * `edit-records.sh`. For the current (v1) format, every file it writes is
 * byte-identical to what the bash libs write, and it reads theirs. Reads
 * validate against strict Zod schemas. Gate writes are atomic and serialized
 * by a sidecar lock.
 */
export { diffSha, type DiffShaOptions } from "./diff-sha.ts";
export {
  EDIT_TOOLS,
  applyPatchRecords,
  formatEditRecords,
  isEditTool,
  normalizeEditRecords,
  type EditRecordsResult,
} from "./edit-records.ts";
export {
  GLOBAL_GATE_KEY,
  clearGateFile,
  readGateFile,
  recordGateFailure,
  type GateRead,
} from "./gate-file.ts";
export { branchSlug } from "./state-git.ts";
export {
  toJqJson,
  withLock,
  writeAtomic,
  type LockOptions,
  type StateOptions,
  type Warn,
} from "./state-io.ts";
export {
  EDIT_OPERATIONS,
  EditRecordSchema,
  GATE_FILE_VERSION,
  GateEntrySchema,
  GateFileSchema,
  TELEMETRY_EXTRAS,
  TELEMETRY_VERSION,
  TelemetryLineSchema,
  isTelemetryEvent,
  type EditRecord,
  type GateEntries,
  type GateEntry,
  type GateFile,
  type TelemetryEvent,
  type TelemetryExtras,
  type TelemetryLine,
} from "./state-schema.ts";
export {
  SWEEP_BRANCH_DIRS,
  SWEEP_DEFAULT_RETENTION_DAYS,
  SWEEP_DEFAULT_TTL_HOURS,
  keptTelemetryLines,
  slugOfStateFile,
  sweepState,
} from "./state-sweeper.ts";
export { TELEMETRY_MAX_LINE_BYTES, telemetryAppend, type TelemetryResult } from "./telemetry.ts";
