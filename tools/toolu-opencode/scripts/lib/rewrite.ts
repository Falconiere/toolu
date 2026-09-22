/** Body rewrites and reference cataloging (#206). */
import { CLAUDE_PLUGIN_ROOT, TOOLU_PLUGIN_ROOT } from "./constants.ts";

export type RewriteNotes = {
  claudePluginRootRewrites: number;
  dotClaudeRefs: string[];
};

/** Match remaining host path tokens after CLAUDE_PLUGIN_ROOT rewrite. */
const DOT_CLAUDE_PATH = /(?:^|[\s"'`(/=])\.claude(?:\/|["'`)\s]|$)/;

export function rewriteBody(body: string): { body: string; notes: RewriteNotes } {
  const parts = body.split(CLAUDE_PLUGIN_ROOT);
  const claudePluginRootRewrites = parts.length - 1;
  const rewritten = parts.join(TOOLU_PLUGIN_ROOT);

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
      dotClaudeRefs: [...new Set(dotClaudeRefs)].sort(),
    },
  };
}
