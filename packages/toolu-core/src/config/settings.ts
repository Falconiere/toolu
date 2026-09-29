/**
 * Plugin settings files (#253): typed loaders for `plugins/toolu/settings/*`,
 * the data the gate modules read. Port of `toolu_settings_dir` and `read_list`
 * (`plugins/toolu/hooks/lib/detect.sh`) plus each consumer's parse. Matching
 * (argv, globs, prefixes) stays with the gate ports.
 */
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
import { isFile } from "./config-files.ts";

import { SETTINGS_FILES } from "./settings-files.ts";

export { settingsDir } from "./settings-dir.ts";
export { SETTINGS_FILES } from "./settings-files.ts";

const COMMENT_OR_BLANK = /^\s*(#|$)/;

/** `read_list FILE`: every line that is not blank or a `#` comment, verbatim; `[]` when absent. */
export function readList(path: string): string[] {
  if (!isFile(path)) {
    return [];
  }
  const text = readFileSync(path, "utf8");
  const lines = text.split("\n");
  if (text.endsWith("\n")) {
    lines.pop();
  }
  return lines.filter((line) => !COMMENT_OR_BLANK.test(line));
}

/** `bash-allowlist.txt`: explicit overrides of deny rules. */
export function bashAllowlist(dir: string): string[] {
  return readList(join(dir, SETTINGS_FILES.bashAllowlist));
}

/** `bash-denylist.txt`: argv-aware deny rules. */
export function bashDenylist(dir: string): string[] {
  return readList(join(dir, SETTINGS_FILES.bashDenylist));
}

/** `commit-prefixes.txt`: allowed Conventional Commits types. */
export function commitPrefixes(dir: string): string[] {
  return readList(join(dir, SETTINGS_FILES.commitPrefixes));
}

/** `protected-files.txt`: extended globs guarded by the protectedFiles gate. */
export function protectedFiles(dir: string): string[] {
  return readList(join(dir, SETTINGS_FILES.protectedFiles));
}

/** `rust-unsafe-exemptions.txt`: crate names allowed to use `unsafe`. */
export function rustUnsafeExemptions(dir: string): string[] {
  return readList(join(dir, SETTINGS_FILES.rustUnsafeExemptions));
}

export type McpBlockEntry = { prefix: string; redirect: string };

/**
 * `mcp-blocklist.txt` as `mcp-blocker.sh` splits it: an optional ` -> <text>`
 * redirect hint after the server prefix; the prefix is whitespace-trimmed and
 * an entry with an empty prefix is dropped.
 */
export function mcpBlocklist(dir: string): McpBlockEntry[] {
  const entries: McpBlockEntry[] = [];
  for (const line of readList(join(dir, SETTINGS_FILES.mcpBlocklist))) {
    const arrow = line.indexOf(" -> ");
    const prefix = (arrow === -1 ? line : line.slice(0, arrow)).trim();
    if (prefix !== "") {
      entries.push({ prefix, redirect: arrow === -1 ? "" : line.slice(arrow + 4) });
    }
  }
  return entries;
}

const CodeEditRulesSchema = z.object({
  rules: z.array(
    z
      .object({
        match: z.string(),
        docs: z.array(z.string()),
        when_path_matches: z.array(z.string()).optional(),
        extra_docs: z.array(z.string()).optional(),
      })
      .strict(),
  ),
});

export type CodeEditRule = {
  match: string;
  docs: string[];
  whenPathMatches: string[];
  extraDocs: string[];
};

export type CodeEditRules = { ok: true; rules: CodeEditRule[] } | { ok: false; reason: string };

/** `code-edit-rules.json`: `{ ok: true, rules: [] }` when absent, `{ ok: false }` when unreadable. */
export function codeEditRules(dir: string): CodeEditRules {
  const path = join(dir, SETTINGS_FILES.codeEditRules);
  if (!isFile(path)) {
    return { ok: true, rules: [] };
  }
  let raw: unknown;
  try {
    raw = JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    return { ok: false, reason: `${path}: ${String(error)}` };
  }
  const parsed = CodeEditRulesSchema.safeParse(raw);
  if (!parsed.success) {
    return { ok: false, reason: `${path}: ${parsed.error.message}` };
  }
  const rules = parsed.data.rules.map((rule) => ({
    match: rule.match,
    docs: rule.docs,
    whenPathMatches: rule.when_path_matches ?? [],
    extraDocs: rule.extra_docs ?? [],
  }));
  return { ok: true, rules };
}
