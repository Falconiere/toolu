/**
 * How a post-edit quality module ends (#265), a port of the `99-finalize.sh`
 * the ts/python/rust-quality bash modules share: violations are recorded in
 * the file's own gate entry and reported, a clean file clears its entry and
 * reports only the advisories.
 */
import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import type { Decision } from "../decision/decision.ts";
import { projectStateRoot } from "../host/host-roots.ts";
import type { RegistryContext } from "../registry/registry-types.ts";
import { clearGateFile, recordGateFailure } from "../state/gate-file.ts";
import type { EditedFile } from "./quality-edit.ts";

/** What a module found: blocking violations, then non-blocking advisories, each in check order. */
export type QualityFindings = {
  readonly errors: readonly string[];
  readonly advisories: readonly string[];
};

/** The gate entry a module owns: its `source` and the `reason` recorded with a failure. */
export type QualityGate = { readonly source: string; readonly reason: string };

/** `$(toolu_project_state_root "$PROJECT_ROOT")/quality-gate-status.json`. */
export function qualityGateFile(ctx: RegistryContext): string {
  const dir = projectStateRoot({ env: ctx.env, host: ctx.host, root: ctx.projectRoot });
  if (dir === undefined) throw new Error("quality: no project state root");
  return join(dir, "quality-gate-status.json");
}

/** `gate_clear_file`: drop `file`'s entry when this module set it. */
export function clearQualityEntry(ctx: RegistryContext, file: EditedFile, source: string): void {
  clearGateFile(qualityGateFile(ctx), file.path, source, { env: ctx.env, host: ctx.host });
}

/**
 * Record or clear `file`'s entry and say what to tell the agent. Each error is
 * one `add_error` line, so the recorded violations end with a newline.
 */
export function settleQuality(
  ctx: RegistryContext,
  file: EditedFile,
  gate: QualityGate,
  findings: QualityFindings,
): Decision {
  if (findings.errors.length > 0) {
    const messages = findings.errors.map((error) => `${error}\n`).join("");
    const gateFile = qualityGateFile(ctx);
    mkdirSync(dirname(gateFile), { recursive: true });
    const state = { env: ctx.env, host: ctx.host };
    recordGateFailure(gateFile, file.path, gate.source, gate.reason, messages, state);
    return {
      kind: "advisory",
      message: `QUALITY VIOLATION — fix before proceeding:\n${messages}`,
    };
  }
  clearQualityEntry(ctx, file, gate.source);
  const advisory = findings.advisories.filter((text) => text !== "").join("\n");
  return advisory === "" ? { kind: "allow" } : { kind: "advisory", message: advisory };
}
