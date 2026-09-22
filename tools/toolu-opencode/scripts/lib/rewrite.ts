/** Body rewrites and reference cataloging (#206). */
import { CLAUDE_PLUGIN_ROOT, TOOLU_PLUGIN_ROOT } from "./constants.ts";

export type RewriteNotes = {
  claudePluginRootRewrites: number;
  dotClaudeRefs: string[];
};

export function rewriteBody(body: string): { body: string; notes: RewriteNotes } {
  const dotClaudeRefs: string[] = [];
  const lines = body.split("\n");
  for (const line of lines) {
    if (line.includes(".claude")) {
      dotClaudeRefs.push(line.trim());
    }
  }

  const parts = body.split(CLAUDE_PLUGIN_ROOT);
  const claudePluginRootRewrites = parts.length - 1;
  const rewritten = parts.join(TOOLU_PLUGIN_ROOT);

  return {
    body: rewritten,
    notes: {
      claudePluginRootRewrites,
      dotClaudeRefs: [...new Set(dotClaudeRefs)].sort(),
    },
  };
}
