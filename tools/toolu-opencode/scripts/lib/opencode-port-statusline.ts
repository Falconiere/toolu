/** OpenCode's native status skill text, derived from the Codex status skill. */
import type { PortEdit } from "./opencode-port-pr-babysit.ts";

export const STATUSLINE_PORTS: Readonly<Record<string, readonly PortEdit[]>> = {
  "plugins/statusline/skills/status/SKILL.md": [
    [
      "quality-gate, push-review, or waiver status in Codex.",
      "quality-gate, push-review, or waiver status in OpenCode.",
    ],
    ["toolu --host codex status", "toolu --host opencode status"],
    ["The explicit host selects Codex's state", "The explicit host selects OpenCode's state"],
    ["values when Codex does not expose them.", "values when OpenCode does not expose them."],
  ],
};
