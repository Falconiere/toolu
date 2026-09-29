/**
 * lint-suppressions — dead code must be removed or wired, never hidden. The
 * compiler and linter find dead declarations; this check owns the independent
 * fact that source must not turn that off: a blanket oxlint/eslint disable, a
 * scoped no-unused-vars disable, or a Rust allow/warn/expect of `dead_code` or
 * the `unused` group. Script and Rust files are lexed separately, so a
 * directive inside a string, template or JSX text is documentation.
 */
import { readdirSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import type { Dirent } from "node:fs";
import type { GuardrailsConfig } from "../config.ts";
import type { CheckContext, Mode } from "../context.ts";
import {
  RUST_CANDIDATE,
  SCRIPT_CANDIDATE,
  rustForbidden,
  scriptForbidden,
} from "../lexer/forbidden.ts";
import { fatal } from "../report.ts";
import { isFile } from "../walk.ts";

const SCRIPT = /\.(ts|tsx|mts|cts|js|jsx|mjs|cjs|astro)$/;
const RUST = /\.rs$/;
/** Pruned only at the package root, exactly like the `find . -path './x' -prune` list. */
const PRUNED = new Set([".git", "node_modules", "dist", "build", "out", "coverage", ".wrangler", ".next", ".expo", "target", "vendor"]);
const PRUNED_SCRIPTS = new Set(["src/route-tree.gen.ts", "worker-configuration.d.ts"]);

function read(root: string, path: string): Buffer {
  try {
    return readFileSync(resolve(root, path));
  } catch {
    return fatal(`lint-suppressions cannot read ${path}`);
  }
}

/** Scan one path; `binarySkip` mirrors grep -I in the repo-mode prefilter. */
function scan(ctx: CheckContext<GuardrailsConfig>, path: string, binarySkip: boolean): void {
  const kind = SCRIPT.test(path) ? "script" : RUST.test(path) ? "rust" : null;
  if (kind === null || !isFile(ctx.root, path)) return;
  const bytes = read(ctx.root, path);
  if (binarySkip && bytes.includes(0)) return;
  const text = bytes.toString("utf8");
  const candidate = kind === "script" ? SCRIPT_CANDIDATE : RUST_CANDIDATE;
  if (!candidate.test(text)) return;
  const forbidden = kind === "script" ? scriptForbidden(path, text) : rustForbidden(text);
  if (!forbidden) return;
  ctx.report.violation(
    "lint-suppressions",
    path,
    "dead-code lint enforcement is disabled",
    "delete the unused code or wire it into the program; do not suppress the lint",
  );
}

function entries(root: string, rel: string): Dirent[] {
  try {
    return readdirSync(resolve(root, rel === "" ? "." : rel), { withFileTypes: true });
  } catch (err: unknown) {
    const reason = err instanceof Error ? err.message : String(err);
    return fatal(`lint-suppressions source inventory failed: ${reason}`);
  }
}

/** Every script and Rust source under the package, untracked edits included. */
function sources(root: string, rel = ""): string[] {
  const out: string[] = [];
  for (const entry of entries(root, rel)) {
    const path = rel === "" ? entry.name : `${rel}/${entry.name}`;
    if (rel === "" && PRUNED.has(entry.name)) continue;
    if (entry.isDirectory()) out.push(...sources(root, path));
    else if (entry.isFile() && (RUST.test(path) || (SCRIPT.test(path) && !PRUNED_SCRIPTS.has(path)))) {
      out.push(path);
    }
  }
  return out;
}

export function lintSuppressions(ctx: CheckContext<GuardrailsConfig>, mode: Mode, path: string): void {
  if (mode === "file") {
    scan(ctx, path, false);
    return;
  }
  for (const source of sources(ctx.root).toSorted()) scan(ctx, source, true);
}
