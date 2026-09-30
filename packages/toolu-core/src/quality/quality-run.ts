/**
 * The per-file flow every post-edit quality module shares (#265), once its
 * project checks passed: resolve the edited file, clear the entry of a
 * deleted or moved-away file, skip files it does not own, then check the file
 * and settle the gate. Mirrors the bash preamble/finalize order exactly.
 */
import type { Decision } from "../decision/decision.ts";
import type { RegistryContext, RegistryHookEvent } from "../registry/registry-types.ts";
import { editedFile, inLinkedWorktree, isRegularFile, type EditedFile } from "./quality-edit.ts";
import {
  clearQualityEntry,
  settleQuality,
  type QualityFindings,
  type QualityGate,
} from "./quality-gate.ts";

export type FileQualitySpec = QualityGate & {
  /** The files this module owns, tested against the path as given (bash `=~`). */
  readonly matches: RegExp;
  /** Leave files in a git linked worktree alone (the TypeScript module does). */
  readonly skipLinkedWorktrees: boolean;
  check(file: EditedFile): QualityFindings;
};

const ALLOW: Decision = { kind: "allow" };

/** Run `spec` over the event's edited file and settle its gate entry. */
export function fileQuality(
  event: RegistryHookEvent,
  ctx: RegistryContext,
  spec: FileQualitySpec,
): Decision {
  const file = editedFile(event, ctx);
  if (file === undefined) return ALLOW;
  if (file.removed) {
    if (spec.matches.test(file.path)) clearQualityEntry(ctx, file, spec.source);
    return ALLOW;
  }
  if (!isRegularFile(file) || !spec.matches.test(file.path)) return ALLOW;
  if (spec.skipLinkedWorktrees && inLinkedWorktree(file, ctx)) return ALLOW;
  return settleQuality(ctx, file, spec, spec.check(file));
}
