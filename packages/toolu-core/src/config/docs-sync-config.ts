/**
 * Docs-sync globs (#253): port of `plugins/toolu/hooks/lib/docs-sync-config.sh`.
 * `docsSync.<key>` (an array) from the merged config replaces the built-in
 * list; an empty or non-array override keeps the default.
 */
import type { LoadedConfig } from "./config-load.ts";
import { section } from "./config-read.ts";

export const DOCS_SYNC_DEFAULTS = {
  surfaces: [
    "README.md",
    "*/README.md",
    "docs/*.md",
    "*/SKILL.md",
    "AGENTS.md",
    "*/AGENTS.md",
    "CLAUDE.md",
    "*/CLAUDE.md",
    "*/workflows/*.md",
  ],
  surfaceExcludes: ["docs/releases/*", "*/docs/releases/*"],
  codeSurfaces: ["*.ts", "*.rs", "*.sh", "*/commands/*", "*plugin.json", "*.config.json"],
} as const;

type DocsSyncKey = keyof typeof DOCS_SYNC_DEFAULTS;

/** jq `-r .[]` output, captured by `$(...)`: one line per element, trailing newlines dropped. */
function jqRawLines(items: readonly unknown[]): string[] {
  const text = items
    .map((item) => (typeof item === "string" ? item : JSON.stringify(item, null, 2)))
    .join("\n")
    .replace(/\n+$/, "");
  return text === "" ? [] : text.split("\n");
}

function globs(config: LoadedConfig, key: DocsSyncKey): string[] {
  const value = section(config, "docsSync")?.[key];
  const lines = Array.isArray(value) ? jqRawLines(value) : [];
  return lines.length > 0 ? lines : [...DOCS_SYNC_DEFAULTS[key]];
}

/** `docs_sync_surfaces`: doc files whose change satisfies the check. */
export function docsSyncSurfaces(config: LoadedConfig): string[] {
  return globs(config, "surfaces");
}

/** `docs_sync_surface_excludes`: doc paths carved back out of the surfaces. */
export function docsSyncSurfaceExcludes(config: LoadedConfig): string[] {
  return globs(config, "surfaceExcludes");
}

/** `docs_sync_code_surfaces`: code files whose change demands a doc touch. */
export function docsSyncCodeSurfaces(config: LoadedConfig): string[] {
  return globs(config, "codeSurfaces");
}
