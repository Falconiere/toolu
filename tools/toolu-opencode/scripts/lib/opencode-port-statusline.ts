/**
 * OpenCode text for the statusline status skill (#359). The source skill is
 * written for Codex; OpenCode's copy runs the report under `shell.env`'s Bun
 * and plugin root, says what the toolu readiness lines mean, and states that
 * OpenCode has no persistent statusline, so the report is its status surface.
 */
import type { PortEdit } from "./opencode-port-pr-babysit.ts";

const RUN = `Run \`TOOLU_HOST_OVERRIDE=codex bun ../../hooks/dist/status.js\` resolved from this
skill directory and return its output verbatim. The explicit override is
required because lifecycle-only plugin variables are not exported to ordinary
skill shell calls.`;

const OPENCODE_RUN = `Run this through bash and return its output verbatim:

\`\`\`bash
# OpenCode
"$TOOLU_BUN" --no-env-file "$TOOLU_PLUGIN_ROOT_STATUSLINE/hooks/dist/status.js"
\`\`\`

\`shell.env\` sets \`TOOLU_BUN\`, \`TOOLU_PLUGIN_ROOT_STATUSLINE\`, \`TOOLU_CONFIG_DIR\` and
\`TOOLU_HOST_OVERRIDE=opencode\` in every bash call. The report starts with toolu's
readiness from this project's startup record: the plugins that started, with
their startup entries, the selection source and any startup notes. A
\`toolu: no startup record\` or \`toolu: unreadable startup record\` line names its
next step; repeat that step to the user. OpenCode has no persistent statusline,
so this report is the status surface: the Claude Code statusline and its setup
command do not apply.`;

export const STATUSLINE_PORTS: Readonly<Record<string, readonly PortEdit[]>> = {
  "plugins/statusline/skills/status/SKILL.md": [
    [
      "quality-gate, comemory, or Jev readiness status in Codex.",
      "quality-gate, comemory, Jev readiness, or toolu plugin readiness status in OpenCode.",
    ],
    [RUN, OPENCODE_RUN],
  ],
};
