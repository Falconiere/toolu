/**
 * banned-deps — a banned package cannot be installed and merely go unimported.
 *
 * oxlint's no-restricted-imports blocks these at the import site; this blocks
 * them at the manifest. A dependency sitting in package.json with no import yet
 * is a decision already made, and the next agent will use it.
 */
import { readFileSync } from "node:fs";
import { join } from "node:path";
import type { RepoFacts } from "../config.ts";
import type { CheckContext, Mode } from "../context.ts";
import { isFile } from "../walk.ts";

const DEP_FIELDS = ["dependencies", "devDependencies", "peerDependencies"];

/** Every declared dependency name; an unreadable manifest declares nothing (jq failed quietly). */
function declared(root: string): Set<string> {
  const names = new Set<string>();
  let doc: unknown;
  try {
    doc = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
  } catch {
    return names;
  }
  if (typeof doc !== "object" || doc === null) return names;
  for (const field of DEP_FIELDS) {
    const deps: unknown = Reflect.get(doc, field);
    if (typeof deps === "object" && deps !== null) {
      for (const name of Object.keys(deps)) names.add(name);
    }
  }
  return names;
}

function escapeRegExp(text: string): string {
  return text.replace(/[\\^$.*+?()[\]{}|]/g, "\\$&");
}

export function bannedDeps(ctx: CheckContext<RepoFacts>, mode: Mode): void {
  const banned = ctx.config.bannedDeps;
  if (mode !== "repo" || banned.length === 0) return;

  if (isFile(ctx.root, "package.json")) {
    const names = declared(ctx.root);
    for (const dep of banned) {
      if (names.has(dep)) {
        ctx.report.violation(
          "banned-deps",
          "package.json",
          `banned dependency: ${dep}`,
          "remove it — one HTTP client (utilities/http.ts over fetch) and one validator (zod)",
        );
      }
    }
  }

  if (isFile(ctx.root, "Cargo.toml")) {
    const cargo = readFileSync(join(ctx.root, "Cargo.toml"), "utf8");
    for (const dep of banned) {
      if (new RegExp(`^[ \\t\\v\\f\\r]*${escapeRegExp(dep)}[ \\t\\v\\f\\r]*=`, "m").test(cargo)) {
        ctx.report.violation(
          "banned-deps",
          "Cargo.toml",
          `banned dependency: ${dep}`,
          "remove it — see LIBRARIES.md for the reasoning behind each ban",
        );
      }
    }
  }
}
