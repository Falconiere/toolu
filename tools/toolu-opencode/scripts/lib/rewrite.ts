/** Body rewrites and reference cataloging (#206). */
import { CLAUDE_PLUGIN_ROOT, TOOLU_PLUGIN_ROOT } from "./constants.ts";

export type RewriteNotes = {
  claudePluginRootRewrites: number;
  explicitReferenceRewrites: number;
  sourcePathRewrites: number;
  claudeConfigRewrites: number;
  dotClaudeRefs: string[];
};

export type SurfaceReferences = {
  invocations: ReadonlyMap<string, string>;
  paths: ReadonlyMap<string, string>;
};

/** Match remaining host path tokens after CLAUDE_PLUGIN_ROOT rewrite. */
const DOT_CLAUDE_PATH = /(?:^|[\s"'`(/=])\.claude(?:\/|["'`)\s]|$)/;

export function rewriteBody(
  body: string,
  references: SurfaceReferences,
): { body: string; notes: RewriteNotes } {
  const parts = body.split(CLAUDE_PLUGIN_ROOT);
  const claudePluginRootRewrites = parts.length - 1;
  let rewritten = parts.join(TOOLU_PLUGIN_ROOT);
  const claudeConfig = "${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}";
  const openCodeConfig = "${TOOLU_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/opencode}";
  const configParts = rewritten.split(claudeConfig);
  const claudeConfigRewrites = configParts.length - 1;
  rewritten = configParts.join(openCodeConfig);
  let sourcePathRewrites = 0;
  for (const [source, destination] of [...references.paths].sort(
    (a, b) => b[0].length - a[0].length,
  )) {
    const pathParts = rewritten.split(source);
    sourcePathRewrites += pathParts.length - 1;
    rewritten = pathParts.join(destination);
  }
  let explicitReferenceRewrites = 0;
  for (const [reference, id] of [...references.invocations].sort(
    (a, b) => b[0].length - a[0].length,
  )) {
    const escaped = reference.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const paired = new RegExp(`\x60([$/])${escaped}\x60\\s+or\\s+\x60([$/])${escaped}\x60`, "g");
    rewritten = rewritten.replace(paired, (match, first: string, second: string) => {
      if (first === second) return match;
      explicitReferenceRewrites += 2;
      return `\x60skill({ name: "${id}" })\x60`;
    });
    const token = new RegExp(`([$/])${escaped}(?![A-Za-z0-9-])`, "g");
    rewritten = rewritten.replace(token, () => {
      explicitReferenceRewrites += 1;
      return `skill({ name: "${id}" })`;
    });
  }

  const dotClaudeRefs: string[] = [];
  for (const line of rewritten.split("\n")) {
    if (DOT_CLAUDE_PATH.test(line)) {
      dotClaudeRefs.push(line.trim());
    }
  }

  return {
    body: rewritten,
    notes: {
      claudePluginRootRewrites,
      explicitReferenceRewrites,
      sourcePathRewrites,
      claudeConfigRewrites,
      dotClaudeRefs: [...new Set(dotClaudeRefs)].sort(),
    },
  };
}
