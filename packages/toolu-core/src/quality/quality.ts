/**
 * `@toolu/core/quality` (#265): what the post-edit language-quality registry
 * modules (ts-quality, then python-quality and rust-quality) share. It ports
 * the preamble and finalize fragments their bash modules repeat: the edited
 * file, delete/move clearing, the linked-worktree skip, the per-file gate entry
 * with its standard message, and the single `ast-grep scan` for structural
 * rules. Each module keeps only its own rules.
 */
export { astGrepScan, type AstGrepHit, type AstGrepScan } from "./quality-ast-grep.ts";
export { editedFile, inLinkedWorktree, isRegularFile, type EditedFile } from "./quality-edit.ts";
export {
  clearQualityEntry,
  qualityGateFile,
  settleQuality,
  type QualityFindings,
  type QualityGate,
} from "./quality-gate.ts";
export { fileQuality, type FileQualitySpec } from "./quality-run.ts";
