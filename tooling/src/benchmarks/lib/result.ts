/**
 * Write and validate benchmark result JSON — the one committed shape
 * (benchmarks/results/README.md).
 *
 * writeResult enforces the never-mix-modes rule: the caller passes a transient
 * `_modes` array (one entry per delta side); if any differ from each other or
 * from tokenizer.mode the write is refused. `_modes` is stripped, so the
 * committed schema stays clean. The file is results/<mechanism>-<tier>-<date>.json.
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { get, list } from "../../json-path.ts";

const REQUIRED = [
  "mechanism",
  "tier",
  "method",
  "tokenizer.mode",
  "tokenizer.source",
  "provenance.date",
  "provenance.commit",
  "provenance.pricing_id",
  "provenance.n_runs",
  "baseline.label",
  "baseline.tokens",
  "treatment.label",
  "treatment.tokens",
  "delta.tokens_pct",
  "delta.abs_tokens",
];

export class ResultError extends Error {}

/** Every problem with a result document; [] when valid. */
function resultProblems(doc: unknown, name: string): string[] {
  if (typeof doc !== "object" || doc === null || Array.isArray(doc))
    return [`invalid JSON in ${name}`];
  const missing = REQUIRED.filter((path) => {
    const value = get(doc, ...path.split("."));
    return value === undefined || value === null || value === "";
  });
  if (missing.length > 0) return [`${name} missing/empty required keys: ${missing.join(",")}`];
  const provenance = get(doc, "provenance");
  if (typeof provenance !== "object" || provenance === null || !("model" in provenance)) {
    return [`${name} missing provenance.model key`];
  }
  return [];
}

export function validateResult(file: string): void {
  if (!existsSync(file)) throw new ResultError(`file not found: ${file}`);
  let doc: unknown;
  try {
    doc = JSON.parse(readFileSync(file, "utf8"));
  } catch {
    throw new ResultError(`invalid JSON in ${file}`);
  }
  const [problem] = resultProblems(doc, file);
  if (problem !== undefined) throw new ResultError(problem);
}

/** Write `result` under `dir`, returning the path. */
export function writeResult(result: Record<string, unknown>, dir: string): string {
  const modes = list(result["_modes"]);
  if (modes.length > 0 && new Set([...modes, get(result, "tokenizer", "mode")]).size !== 1) {
    throw new ResultError("mixed tokenizer modes in one delta; refusing to write");
  }
  const clean = Object.fromEntries(Object.entries(result).filter(([key]) => key !== "_modes"));
  const [mechanism, tier, date] = [
    get(clean, "mechanism"),
    get(clean, "tier"),
    get(clean, "provenance", "date"),
  ];
  if (
    typeof mechanism !== "string" ||
    typeof tier !== "string" ||
    typeof date !== "string" ||
    [mechanism, tier, date].includes("")
  ) {
    throw new ResultError("result needs mechanism, tier, and provenance.date");
  }
  mkdirSync(dir, { recursive: true });
  const out = join(dir, `${mechanism}-${tier}-${date}.json`);
  writeFileSync(out, `${JSON.stringify(clean, null, 2)}\n`);
  validateResult(out);
  return out;
}
