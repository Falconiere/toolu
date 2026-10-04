/** Shared constants for OpenCode surface generation (#206). */
export const GENERATED_SEGMENT = "tools/toolu-opencode/generated";

export const CLAUDE_PLUGIN_ROOT = "${CLAUDE_PLUGIN_ROOT}";

/** The `@toolu/opencode` package directory, which holds `generated/` (#343). */
export const TOOLU_OPENCODE_ROOT = "${TOOLU_OPENCODE_ROOT}";

export const STRIPPED_FRONTMATTER_KEYS = new Set(["disable-model-invocation"]);

export const ARTIFACT_KIND_ORDER = ["agent", "command", "skill"] as const;

export type ArtifactKind = (typeof ARTIFACT_KIND_ORDER)[number];

export const EXCLUDED_SURFACES = {
  "statusline/commands/setup.md": {
    reason:
      "Claude Code statusLine setting in settings.json; OpenCode has no statusline setting, so the persistent statusline is host-specific. Use the statusline-status skill.",
    owner: "OP-25 (#359)",
  },
} as const;

export const COMMAND_SKILL_TARGETS: Record<string, string> = {
  "epic-orchestrator:epic": "epic-orchestrator:epic-orchestrator",
};
