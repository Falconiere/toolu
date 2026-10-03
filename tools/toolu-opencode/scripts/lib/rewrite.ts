/** Body rewrites and reference cataloging (#206). */
import { posix } from "node:path";
import { pluginRootVar } from "../../src/host/runtime-env.ts";
import { CLAUDE_PLUGIN_ROOT, TOOLU_OPENCODE_ROOT } from "./constants.ts";

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

/** Whose surface is being rewritten: its plugin, and its id when it is a skill. */
export type RewriteOwner = { plugin: string; skillId?: string };

const OPENCODE_CONFIG = "${TOOLU_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/opencode}";

/** Jev's Codex assignment and the commented other-host line after it, in either source layout. */
const JEV_HOST_PAIR =
  /# Codex[^\n]*\nJEV="\$\{TOOLU_CONFIG_DIR:-\$\{CODEX_HOME:-\$HOME\/\.codex\}\}\/jev\/jev\.sh"\n(?:# Claude Code[^\n]*\n)?# JEV="[^"\n]*"/g;

/** Jev on OpenCode (#350): the one wrapper `shell.env` resolves, run by a Bun that ignores `.env`. */
export function opencodeJev(text: string): string {
  return text
    .replace(JEV_HOST_PAIR, `# OpenCode\nJEV="${OPENCODE_CONFIG}/jev/jev.sh"`)
    .replaceAll('"$JEV_BUN" "$JEV"', '"$JEV_BUN" --no-env-file "$JEV"');
}

export function rewriteBody(
  body: string,
  references: SurfaceReferences,
  owner: RewriteOwner,
): { body: string; notes: RewriteNotes } {
  const { skillId } = owner;
  // Each plugin's own root, as `${CLAUDE_PLUGIN_ROOT}` is on Claude: shell.env sets one per enabled plugin.
  const parts = body.split(CLAUDE_PLUGIN_ROOT);
  const claudePluginRootRewrites = parts.length - 1;
  let rewritten = parts.join(`\${${pluginRootVar(owner.plugin)}}`);
  const claudeConfig = "${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}";
  const configParts = rewritten.split(claudeConfig);
  const claudeConfigRewrites = configParts.length - 1;
  rewritten = configParts.join(OPENCODE_CONFIG);
  rewritten = rewritten.replace("on Claude\nCode. Ordinary", "on OpenCode.\nOrdinary");
  let sourcePathRewrites = 0;
  for (const [source, destination] of [...references.paths].sort(
    (a, b) => b[0].length - a[0].length,
  )) {
    const pathParts = rewritten.split(source);
    sourcePathRewrites += pathParts.length - 1;
    rewritten = pathParts.join(destination);
  }
  if (skillId) {
    if (skillId === "toolu-review-review") {
      rewritten = rewritten.replace("# Claude Code\n", "# OpenCode\n");
      rewritten = rewritten.replace("TOOLU_HOST_OVERRIDE=claude", "TOOLU_HOST_OVERRIDE=opencode");
    }
    if (skillId === "jev-jev") rewritten = opencodeJev(rewritten);
    if (skillId === "statusline-status") {
      rewritten = rewritten.replace("in Codex.", "in OpenCode.");
      rewritten = rewritten.replace(
        "TOOLU_HOST_OVERRIDE=codex bun ../../hooks/dist/status.js",
        "TOOLU_HOST_OVERRIDE=opencode bun ../../../plugins/statusline/hooks/dist/status.js",
      );
    }
    const modelRouting = `${TOOLU_OPENCODE_ROOT}/generated/skills/toolu-orchestrator/references/model-routing.md`;
    const relativeRouting = posix.relative(
      `skills/${skillId}`,
      "skills/toolu-orchestrator/references/model-routing.md",
    );
    rewritten = rewritten.replaceAll(`\`${modelRouting}\``, `\`${relativeRouting}\``);
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
    if (reference === "brainstorm:brainstorm") {
      rewritten = rewritten.replaceAll(`\`${reference}\``, `\`${id}\``);
    }
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
