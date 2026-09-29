/**
 * One `ast-grep scan --inline-rules <rules> --json <file>` for every structural
 * rule of a quality module (#265), and the hits flattened as the bash modules'
 * `jq` did: one `<file>:<line>:<source line>` excerpt per matched source line,
 * tagged with its rule. A crash, stderr output or output that is not the
 * documented JSON array is reported with its stage, never read as "no hits".
 */
import { constants } from "node:os";
import { childEnv, envValue } from "../host/host-name.ts";
import { isJsonObject } from "../config/config-load.ts";
import type { RegistryContext } from "../registry/registry-types.ts";
import type { EditedFile } from "./quality-edit.ts";

/** One matched source line: its rule, its 1-based line number and `<file>:<line>:<source line>`. */
export type AstGrepHit = {
  readonly ruleId: string;
  readonly line: number;
  readonly excerpt: string;
};

export type AstGrepScan =
  | { readonly kind: "missing" }
  /** `empty`: stdout held only whitespace, which `jq` reads as no hits. */
  | { readonly kind: "ok"; readonly hits: readonly AstGrepHit[]; readonly empty: boolean }
  | {
      readonly kind: "failed";
      readonly stage: "ast-grep" | "parse";
      readonly exitCode: number;
      /** First stderr line, capped at 200 characters, as `head -n 1 | cut -c1-200`. */
      readonly stderrFirst: string;
    };

/** jq string interpolation `\(value)`: strings raw, everything else as JSON. */
function interpolated(value: unknown): string {
  return typeof value === "string" ? value : JSON.stringify(value ?? null);
}

/** One match's excerpt lines, or undefined where the bash `jq` program would fail. */
function matchHits(match: unknown): AstGrepHit[] | undefined {
  if (!isJsonObject(match)) return undefined;
  const lines =
    match.lines === undefined || match.lines === null || match.lines === false ? "" : match.lines;
  const range = match.range;
  const start = isJsonObject(range) && isJsonObject(range.start) ? range.start.line : undefined;
  if (typeof lines !== "string" || typeof start !== "number") return undefined;
  // jq's `"" | split("\n")` is `[]`, not `[""]`.
  const texts = lines === "" ? [] : lines.split("\n");
  const ruleId = interpolated(match.ruleId);
  const file = interpolated(match.file);
  return texts.map((text, key) => ({
    ruleId,
    line: start + 1 + key,
    excerpt: `${file}:${String(start + 1 + key)}:${text}`,
  }));
}

function parseHits(stdout: string): AstGrepHit[] | undefined {
  let doc: unknown;
  try {
    doc = JSON.parse(stdout);
  } catch {
    return undefined;
  }
  if (!Array.isArray(doc)) return undefined;
  const hits: AstGrepHit[] = [];
  for (const match of doc) {
    const flat = matchHits(match);
    if (flat === undefined) return undefined;
    hits.push(...flat);
  }
  return hits;
}

/** The status bash's `$?` reports: the exit code, or 128 + the signal number. */
function exitStatus(res: { exitCode: number | null; signalCode?: string | null }): number {
  if (res.exitCode !== null) return res.exitCode;
  const signals: Readonly<Record<string, number>> = constants.signals;
  return 128 + (signals[res.signalCode ?? ""] ?? 0);
}

/** Run every rule over `file` in one ast-grep process; `missing` when `ast-grep` is not on PATH. */
export function astGrepScan(file: EditedFile, rules: string, ctx: RegistryContext): AstGrepScan {
  const cwd = ctx.cwd ?? process.cwd();
  const bin = Bun.which("ast-grep", { PATH: envValue(ctx.env, "PATH") ?? "", cwd });
  if (bin === null) return { kind: "missing" };
  const res = Bun.spawnSync([bin, "scan", "--inline-rules", rules, "--json", file.path], {
    cwd,
    env: childEnv(ctx.env),
    stdout: "pipe",
    stderr: "pipe",
  });
  const stdout = res.stdout.toString();
  const stderr = res.stderr.toString();
  const exitCode = exitStatus(res);
  if (exitCode !== 0 || stderr !== "") {
    const stderrFirst = (stderr.split("\n")[0] ?? "").slice(0, 200);
    return { kind: "failed", stage: "ast-grep", exitCode, stderrFirst };
  }
  if (stdout.trim() === "") return { kind: "ok", hits: [], empty: true };
  const hits = parseHits(stdout);
  if (hits === undefined) return { kind: "failed", stage: "parse", exitCode, stderrFirst: "" };
  return { kind: "ok", hits, empty: false };
}
