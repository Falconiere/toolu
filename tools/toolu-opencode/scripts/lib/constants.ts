/** Shared constants for OpenCode surface generation (#206). */
export const DEFAULT_ENABLED = ["toolu", "pr-babysit", "epic-orchestrator"] as const;

export const GENERATED_SEGMENT = "tools/toolu-opencode/generated";

export const CLAUDE_PLUGIN_ROOT = "${CLAUDE_PLUGIN_ROOT}";

export const TOOLU_PLUGIN_ROOT = "${TOOLU_PLUGIN_ROOT}";

export const STRIPPED_FRONTMATTER_KEYS = new Set(["disable-model-invocation"]);

export const ARTIFACT_KIND_ORDER = ["agent", "command", "skill"] as const;

export type ArtifactKind = (typeof ARTIFACT_KIND_ORDER)[number];
