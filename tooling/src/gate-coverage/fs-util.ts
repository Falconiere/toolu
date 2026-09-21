/** Small filesystem helpers for inventory discovery. */
import { existsSync, readdirSync } from "node:fs";
import { basename, join, relative } from "node:path";
import type { Kind } from "./types.ts";
import { ROOT } from "./paths.ts";

export function fail(msg: string): never {
  console.error(`gate-coverage-inventory: ${msg}`);
  process.exit(1);
}

export function rel(p: string): string {
  return relative(ROOT, p).split("\\").join("/");
}

export function listDirNames(dir: string): string[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true })
    .filter((e) => e.isDirectory())
    .map((e) => e.name)
    .toSorted();
}

export function listShFiles(dir: string, pattern?: RegExp): string[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true })
    .filter((e) => e.isFile() && e.name.endsWith(".sh") && (!pattern || pattern.test(e.name)))
    .map((e) => join(dir, e.name))
    .toSorted();
}

export function normalizeCommand(command: string): string {
  let c = command.trim();
  if ((c.startsWith('"') && c.endsWith('"')) || (c.startsWith("'") && c.endsWith("'"))) {
    c = c.slice(1, -1);
  }
  return c.replace("${CLAUDE_PLUGIN_ROOT}/", "").replace("${PLUGIN_ROOT}/", "");
}

function sanitizeIdPart(s: string): string {
  return s.replace(/["']/g, "").replace(/\s+/g, "_");
}

export function makeId(
  plugin: string,
  kind: Kind,
  event: string,
  commandOrModule: string,
  matcher = "",
): string {
  const base = sanitizeIdPart(basename(commandOrModule));
  const m = matcher ? `:${sanitizeIdPart(matcher).slice(0, 40)}` : "";
  return `${plugin}:${kind}:${event}:${base}${m}`;
}
