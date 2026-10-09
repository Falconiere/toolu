/**
 * OpenCode text for the toolu and toolu-review (#358), the brainstorm and
 * delivery-flow (#355), the epic-orchestrator (#356), pr-babysit's (#357,
 * `opencode-port-pr-babysit.ts`) and the statusline status skill's (#359,
 * `opencode-port-statusline.ts`) surfaces, keyed by
 * repo-relative source path. Each `from` must occur exactly `count` times
 * (default 1) in that source, so a source edit that moves an anchor fails
 * generation instead of shipping Claude Code or Codex text to OpenCode.
 */

import { PR_BABYSIT_PORTS, type PortEdit } from "./opencode-port-pr-babysit.ts";
import { STATUSLINE_PORTS } from "./opencode-port-statusline.ts";

const AGENT_LIST =
  "`toolu-quick-task`, `toolu-deep-explore`, `toolu-research-agent`, `toolu-implementer` and `toolu-architect`";

const MODEL_NOTE =
  "On OpenCode the `task` tool takes no model argument: each agent runs `agent.<id>.model` from your `opencode.json`, else the session's model.";

const TOOLU_SCRIPTS = '"$TOOLU_PLUGIN_ROOT_TOOLU/scripts';

const skill = (id: string): string => `\`skill({ name: "${id}" })\``;
const BABYSIT = skill("pr-babysit-babysit-73c340c6");
const REVIEW = skill("toolu-review-review");

const TIER_AGENTS =
  "`toolu-quick-task` (`haiku`), `toolu-implementer` (`sonnet`), `toolu-architect` (`opus`, `fable`) or `general` (`inherit`)";

// delivery-flow ships byte-identical private copies of these two toolu files.
const MODEL_ROUTING_PORT: readonly PortEdit[] = [
  [
    "(`plugins/toolu/hooks/docs/model-routing.md`)",
    "(`plugins/toolu/hooks/docs/model-routing-opencode.md` on OpenCode)",
  ],
  [
    "reasoning-effort pair in `models.codex.<class>`.",
    `reasoning-effort pair in \`models.codex.<class>\`. ${MODEL_NOTE} Each class
routes to an agent instead (see Pre-tiered agents).`,
  ],
  [
    `Pinning the tier in the agent's own frontmatter is stronger than remembering to
pass \`model:\`, so prefer these when one fits:

| Agent | Tier | Job |
|---|---|---|
| \`toolu:quick-task\` | \`haiku\` | Mechanical lookups and bounded mechanical edits |
| \`toolu:deep-explore\` | \`sonnet\` | Structural exploration via ast-grep |
| \`toolu:research-agent\` | \`sonnet\` | External docs / web research |
| \`toolu:implementer\` | \`sonnet\` | One bounded plan step + its tests |
| \`toolu:architect\` | \`opus\` | Design, trade-offs, synthesis (read-only) |

For anything else, route explicitly with the active host's delegation interface.
Leaving routing unset inherits host defaults.`,
    `Pass one of these as the \`task\` tool's \`subagent_type\` when it fits. On OpenCode
they carry no model of their own; set \`agent.<id>.model\` to pin one.

| Agent | Claude Code tier | Job |
|---|---|---|
| \`toolu-quick-task\` | \`haiku\` | Mechanical lookups and bounded mechanical edits |
| \`toolu-deep-explore\` | \`sonnet\` | Structural exploration via ast-grep |
| \`toolu-research-agent\` | \`sonnet\` | External docs / web research |
| \`toolu-implementer\` | \`sonnet\` | One bounded plan step + its tests |
| \`toolu-architect\` | \`opus\` | Design, trade-offs, synthesis (read-only) |

For anything else, use OpenCode's \`general\` or \`explore\` agent.`,
  ],
  [
    `**Limit:** Claude config remaps the rubric but cannot rewrite a pre-built agent's
frontmatter. Codex custom-agent files also take precedence over class routing;
run \`$toolu:setup\` after plugin upgrades to install current profile templates.`,
    `**Limit:** on OpenCode, \`models\` remaps the session text only; an agent's model
comes from \`agent.<id>.model\` in \`opencode.json\`. There are no agent profiles to
install.`,
  ],
];

const SEMANTIC_JUDGMENTS_PORT: readonly PortEdit[] = [
  [
    "Read the installed `jev` skill for host path, CLI, question contract, and",
    'Load `skill({ name: "jev-jev" })` when Jev is enabled, for the wrapper path, CLI, question contract, and',
  ],
];

const DELIVERY = "plugins/delivery-flow/skills/delivery-flow";
const EPIC = "plugins/epic-orchestrator/skills/epic-orchestrator";

export const OPENCODE_PORTS: Readonly<Record<string, readonly PortEdit[]>> = {
  ...PR_BABYSIT_PORTS,
  ...STATUSLINE_PORTS,
  "plugins/brainstorm/skills/brainstorm/SKILL.md": [
    [
      "Delegate only when the search needs a broad map, on a\nread-only exploration tier; keep the final trade-off decision in the main\nthread on the most capable tier.",
      'Delegate only when the search needs a broad map, to the `toolu-deep-explore`\nagent (`task` with `subagent_type: "toolu-deep-explore"`) when it is listed, else\nOpenCode\'s `explore`; keep the final trade-off decision in the main thread.',
    ],
    [
      "When Jev is installed, call it to compare",
      `When Jev is enabled, load ${skill("jev-jev")} and call it to compare`,
    ],
    [
      "Use\nthe host's structured-choice tool: `AskUserQuestion` in Claude Code,\n`request_user_input` in Codex when available; otherwise ask one concise plain\nquestion.",
      "Use\nOpenCode's `question` tool when it is listed; otherwise ask one concise plain\nquestion.",
    ],
  ],
  [`${DELIVERY}/SKILL.md`]: [
    ["babysit on Claude Code or Codex.", "babysit on OpenCode."],
    ["`pr-babysit:babysit`", BABYSIT, 3],
    [
      "Use the active host's `/delivery-flow:delivery-flow` or `$delivery-flow:delivery-flow` invocation as appropriate.",
      `On OpenCode it is ${skill("delivery-flow-delivery-flow")}.`,
    ],
    [
      `When
\`toolu\` is not on the agent's \`PATH\`, the toolu plugin's session start names the
absolute path or the install command to use instead.`,
      `On
OpenCode, enabling delivery-flow enables toolu, whose session start reports a
missing \`toolu\`. When bash cannot run \`toolu\`, toolu is not ready in this
session: stop and name that prerequisite.`,
    ],
    [
      "Invoke the active host's `brainstorm:brainstorm` (`/brainstorm:brainstorm` or `$brainstorm:brainstorm`) in its Delivery mode",
      `Load ${skill("brainstorm-brainstorm")} in its Delivery mode`,
    ],
    [
      "`toolu-review:review` with complete version: 2 state",
      `${REVIEW} with complete version: 2 state`,
    ],
    [
      "the installed `brainstorm`, `toolu`, `toolu-review`, and `pr-babysit` skills",
      `a \`toolu\` that bash can run, the ${skill("brainstorm-brainstorm")}, ${REVIEW} and ${BABYSIT} skills in the \`skill\` tool's list`,
    ],
  ],
  [`${DELIVERY}/references/execution.md`]: [
    [
      "4. **Land it clean.** A PostToolUse quality gate runs on every TS/Rust edit. If it reports a violation the gate goes **failing** and blocks further edits until fixed — fix immediately; do not pile on more changes.",
      "4. **Land it clean.** On OpenCode, toolu's `tool.execute.after` check runs on every TS/Rust edit and appends any violation to that edit's result. The gate then goes **failing** and blocks the next `git commit` and `git push` until fixed — fix immediately; do not pile on more changes.",
    ],
    [
      "hand that step to a subagent on that model (`toolu:quick-task` / `toolu:implementer` / `toolu:architect`, or an explicit `model:`).",
      `hand that step to its agent as the \`task\` tool's \`subagent_type\`: ${TIER_AGENTS}. \`task\` takes no model argument.`,
    ],
    [
      "Change the hypothesis (`systematic-debugging`), don't retry harder.",
      `Change the hypothesis (${skill("toolu-debug")}), don't retry harder.`,
    ],
    [
      "the required `pr-babysit` plugin is not installed or its `pr-babysit:babysit` skill is unavailable.",
      `the required \`pr-babysit\` plugin is not enabled, or ${BABYSIT} is not in the \`skill\` tool's list.`,
    ],
    [
      "4. Run the active host's `toolu-review:review` invocation (see [host-mapping.md](host-mapping.md)) against",
      `4. Load ${REVIEW} and run it against`,
    ],
    [
      "3. Invoke the active host's `pr-babysit:babysit` (see [host-mapping.md](host-mapping.md)) with no arguments.",
      `3. Load ${BABYSIT} and run it with no arguments.`,
    ],
  ],
  [`${DELIVERY}/references/ledger.md`]: [
    [
      "- `model`: `haiku`, `sonnet`, `opus`, `fable`, or `inherit`.",
      `- \`model\`: \`haiku\`, \`sonnet\`, \`opus\`, \`fable\`, or \`inherit\`. On OpenCode these
  are tier labels, not models: execution passes ${TIER_AGENTS} as the \`task\`
  tool's \`subagent_type\`, which takes no model argument.`,
    ],
  ],
  [`${DELIVERY}/references/model-routing.md`]: MODEL_ROUTING_PORT,
  [`${DELIVERY}/references/semantic-judgments.md`]: SEMANTIC_JUDGMENTS_PORT,
  "plugins/toolu-review/skills/review/SKILL.md": [
    [
      "   toolu review write-state --findings-count 0 \\",
      "   # OpenCode\n   TOOLU_HOST_OVERRIDE=opencode toolu review write-state --findings-count 0 \\",
    ],
    [
      "the active host's `<repo root>/.claude/tmp/push-review/`,\n   `<repo root>/.codex/tmp/push-review/` or\n   `<repo root>/.opencode/tmp/push-review/` path atomically",
      "OpenCode's `<repo root>/.opencode/tmp/push-review/` path atomically",
    ],
  ],
  "plugins/toolu/skills/debug/SKILL.md": [
    [
      "in the toolu layout — see the `test` skill (TS `__tests__/`, Rust `tests/`).",
      "in the toolu layout (TS `__tests__/`, Rust `tests/`).",
    ],
    ["plugins/toolu/scripts/debug-testfail.ts", `${TOOLU_SCRIPTS}/debug-testfail.ts"`, 3],
    ["plugins/toolu/scripts/debug-stack.ts", `${TOOLU_SCRIPTS}/debug-stack.ts"`, 2],
    ["plugins/toolu/scripts/debug-log.ts", `${TOOLU_SCRIPTS}/debug-log.ts"`],
    [
      "1. The Sentry MCP's fetch tools only appear **after** OAuth — discover them at runtime with `ToolSearch` (e.g. query `+Sentry issue event`); do not assume tool names. If only `mcp__claude_ai_Sentry__authenticate` is present, the user hasn't connected it.",
      "1. OpenCode lists MCP tools with the others, named `<server>_<tool>` for each server under `mcp` in `opencode.json`; there is no tool search. Look for a Sentry server's fetch tools by that prefix; do not assume tool names. If none is listed, the user hasn't configured or authenticated it (`opencode mcp auth <server>`).",
    ],
  ],
  "plugins/toolu/skills/orchestrator/SKILL.md": [
    [
      "they do not recursively\ndelegate unless the task explicitly requires a nested workflow.",
      "they do not recursively\ndelegate. OpenCode's default `subagent_depth` of 1 refuses a `task` call made from inside a subagent.",
    ],
    [
      "Prefer a **tier-pinned** agent when one fits — its frontmatter fixes the model, so routing can't be forgotten:",
      "Prefer a toolu agent when one fits. Pass its name as the `task` tool's `subagent_type`:",
    ],
    [
      `- **\`toolu:quick-task\` / Codex \`quick-task\`** — mechanical lookups and listings.
- **\`toolu:deep-explore\` / Codex \`deep-explore\`** — structural exploration.
- **\`toolu:research-agent\` / Codex \`research-agent\`** — external research.
- **\`toolu:implementer\` / Codex \`implementer\`** — one bounded plan step and tests.
- **\`toolu:architect\` / Codex \`architect\`** — design and synthesis, read-only.
- **\`Explore\`** — broad read-only fan-out search when you need the conclusion, not file dumps.
- **\`Plan\`** — design an implementation strategy for a non-trivial change.
- **\`general-purpose\`** — multi-step research/execution that doesn't fit a specific agent; set \`model:\` yourself.`,
      `- **\`toolu-quick-task\`** — mechanical lookups and listings.
- **\`toolu-deep-explore\`** — structural exploration.
- **\`toolu-research-agent\`** — external research.
- **\`toolu-implementer\`** — one bounded plan step and tests.
- **\`toolu-architect\`** — design and synthesis, read-only.
- **\`explore\`** — OpenCode's built-in read-only search agent, when you need the conclusion, not file dumps.
- **\`general\`** — OpenCode's built-in agent for multi-step research/execution that doesn't fit a specific agent.

OpenCode's \`plan\` is a primary agent, not a subagent: plan in the main thread instead.`,
    ],
    [
      `Select the matching preconfigured
agent or pass the active host's explicit model and reasoning settings. Omitting
routing inherits host defaults, which may be inappropriate for the task.

| Class | Claude default | Codex default | Belongs here |
|---|---|---|---|
| mechanical | \`haiku\` | Luna / medium | lookups, listings, formatting, one command |
| exploration | \`sonnet\` | Terra / medium | read-only search across many files |
| implementation | \`sonnet\` | Terra / medium | a bounded decided edit + tests |
| review | \`sonnet\` | Terra / high | diff review, audits |
| synthesis | \`opus\` | Sol / high | reconciling findings |
| architecture | \`opus\` | Sol / high | design and hard-to-reverse calls |`,
      `${MODEL_NOTE} You route by choosing the agent.

| Class | OpenCode agent | Belongs here |
|---|---|---|
| mechanical | \`toolu-quick-task\` | lookups, listings, formatting, one command |
| exploration | \`toolu-deep-explore\`, \`toolu-research-agent\` | read-only search across many files |
| implementation | \`toolu-implementer\` | a bounded decided edit + tests |
| review | \`general\` | diff review, audits |
| synthesis | \`toolu-architect\` | reconciling findings |
| architecture | \`toolu-architect\` | design and hard-to-reverse calls |`,
    ],
  ],
  "plugins/toolu/skills/orchestrator/references/model-routing.md": MODEL_ROUTING_PORT,
  "plugins/toolu/skills/deep-research/SKILL.md": [
    ["that stays with `research-agent`", "that stays with `toolu-research-agent`"],
    ["that is `deep-explore`'s job", "that is `toolu-deep-explore`'s job"],
    [
      "Workers run on sonnet, synthesis stays on the frontier tier",
      "Workers run as `toolu-research-agent` tasks, synthesis stays in the main thread",
    ],
    [
      "One `research-agent` (sonnet) per question, launched in parallel.",
      'One `task` with `subagent_type: "toolu-research-agent"` per question, launched in parallel.',
    ],
    [
      "One `research-agent` (sonnet) per finding set",
      "One `toolu-research-agent` task per finding set",
    ],
  ],
  "plugins/toolu/skills/setup/SKILL.md": [
    [
      `\`toolu setup agents\` manages \`quick-task\`, \`deep-explore\`, \`research-agent\`, \`implementer\`, and \`architect\`.

1. Run \`toolu setup agents preview\` and show the exact plan.
2. For installs and managed upgrades, run \`toolu setup agents install\`.
3. If preview reports an unmanaged conflict, inspect only the named file and
   ask for explicit confirmation before running \`toolu setup agents install\`
   with \`--force\`. The command creates a timestamped backup before replacement.
4. For removal, show preview and ask for explicit confirmation before
   \`toolu setup agents remove\` with \`--yes\`. Add \`--force\` only after separately
   confirming any unmanaged conflict. Removal moves profiles into a timestamped
   backup.
5. Report the backup path and tell the user to restart Codex so agent profiles
   reload.
6. Run \`toolu doctor\` and \`toolu config validate\`, and present their output.

Never edit agent files by hand or infer confirmation from the original setup
request when a conflict or removal is involved.`,
      `On OpenCode there is nothing to install. The toolu plugin's \`config\` hook
registers its five agents at every start: ${AGENT_LIST}. Codex agent profiles do
not apply to OpenCode.

Tell the user that, then offer the OpenCode equivalents:

- **Pin a model:** set \`agent.<id>.model\` (for example
  \`agent.toolu-quick-task.model\`) in \`opencode.json\`. toolu's prompt and
  permissions stay.
- **Drop an agent:** set \`agent.<id>.disable\` to \`true\`.
- **Update:** \`npx @toolu/plugins update --host opencode\`, then restart OpenCode.

\`toolu setup agents\` refuses to run on OpenCode: it exits 2
with this explanation and writes nothing. Do not edit Codex profiles from here.`,
    ],
  ],
  "plugins/toolu/workflows/semantic-judgments.md": SEMANTIC_JUDGMENTS_PORT,
  "plugins/toolu/workflows/commit.md": [
    [
      "1. Delegate this bounded workflow to the configured `quick-task` mechanical\n   agent when that agent is available.",
      '1. Delegate this bounded workflow to the `toolu-quick-task` agent (`task` with\n   `subagent_type: "toolu-quick-task"`) when it is listed.',
    ],
  ],
  "plugins/toolu/workflows/review-and-commit.md": [
    [
      "Prefer the installed toolu reviewer (`toolu-review:review` in the active host's\ninvocation syntax).",
      'Prefer the installed toolu reviewer (`skill({ name: "toolu-review-review" })`).',
    ],
  ],
  [`${EPIC}/SKILL.md`]: [
    [
      "a background watcher wakes you only when a\ndecision is needed.",
      "a bounded watcher returns only when a\ndecision is needed.",
    ],
    [
      '# Claude / Cursor Agent: CLAUDE_PLUGIN_ROOT. Codex: PLUGIN_ROOT.\n# OpenCode: TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR (its generated surface rewrites CLAUDE_PLUGIN_ROOT to it).\nROOT="${CLAUDE_PLUGIN_ROOT}"\nROOT="${ROOT:-${PLUGIN_ROOT:-${TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR}}}"',
      '# OpenCode\'s bash sets TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR while the plugin is enabled.\nROOT="${TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR:?epic-orchestrator is not enabled in this OpenCode session}"',
    ],
    [
      "- Your skill list includes `delivery-flow:delivery-flow` and its `toolu`,\n  `toolu-review`, `pr-babysit`, and `brainstorm` dependencies (or the OpenCode-generated equivalents). Workers run as\n  herdr agents and need those plugins installed in every host in `--hosts`\n  (`npx @toolu/plugins install delivery-flow --host codex`, and so on).",
      `- These load through the \`skill\` tool: ${skill("delivery-flow-delivery-flow")},\n  ${skill("brainstorm-brainstorm")}, ${REVIEW} and ${BABYSIT}. Workers run as\n  herdr agents and need those plugins installed and enabled in every host in \`--hosts\`.`,
    ],
    ["## 3. Watch (background)", "## 3. Watch"],
    [
      'bun "$S/epic-watch.ts" --state-dir <state_dir>        # Bash with run_in_background: true\n```\n\nThen end your turn. The watcher polls once a minute and costs nothing while\nit waits. When it exits, you are re-invoked with JSON events. Keep exactly one\nwatcher running. After you handle the events, start it again.',
      'bun "$S/epic-watch.ts" --state-dir <state_dir> --max-wait 480   # bash tool with timeout: 600000\n```\n\nOpenCode\'s bash tool has no background mode, and nothing wakes a finished\nturn. So run the watcher in the foreground with a bounded wait: it polls once a\nminute and returns JSON events, or a `heartbeat` after 480 seconds. Handle the\nevents, then run it again in the same turn, and keep that loop going while any\nissue is active. Keep exactly one watcher running.',
    ],
    [
      "| `heartbeat` | Re-run the graph (`--save`).",
      "| `heartbeat` | About every 8 minutes on OpenCode. When about 45 minutes have passed since the last graph run, re-run the graph (`--save`); otherwise just run the watcher again.",
    ],
    [
      "- **Stop.** Stop the watcher task only.",
      "- **Stop.** Stop running the watcher (interrupt a watcher bash call in flight).",
    ],
  ],
  [`${EPIC}/references/worker-brief.md`]: [
    [
      '- DELIVERY, BABYSIT, DEBUG: the host\'s skill invocation, e.g. for delivery-flow\n  `/delivery-flow:delivery-flow` (Claude Code), `$delivery-flow:delivery-flow`\n  (Codex), "the `delivery-flow:delivery-flow` skill" (Cursor Agent),\n  `skill({ name: "delivery-flow-delivery-flow" })` (OpenCode, the generated\n  skill name from hosts.ts OPENCODE_SKILL_IDS). See hosts.ts skillRef.',
      `- DELIVERY, BABYSIT, DEBUG: the worker host's skill invocation (hosts.ts\n  skillRef); an OpenCode worker gets ${skill("delivery-flow-delivery-flow")},\n  the generated skill name from hosts.ts OPENCODE_SKILL_IDS.`,
    ],
  ],
  "plugins/toolu/agents/research-agent.md": [
    ["`WebSearch` and `WebFetch`", "`websearch` and `webfetch`"],
  ],
};

/** Throw naming the source and anchor unless `from` occurs `count` times in `text`. */
function expectCount(sourcePath: string, text: string, from: string, count: number): void {
  const found = text.split(from).length - 1;
  if (found !== count) {
    const anchor = from.split("\n")[0]?.slice(0, 80) ?? "";
    throw new Error(
      `opencode port: ${sourcePath}: expected ${count} match(es), found ${found}: ${anchor}`,
    );
  }
}

/** One port edit, or an error naming the source and anchor when a count differs. */
function applyEdit(sourcePath: string, text: string, edit: PortEdit): string {
  if ("cut" in edit) {
    const [start, end] = edit.cut;
    expectCount(sourcePath, text, start, 1);
    const from = text.indexOf(start);
    if (end === null) return text.slice(0, from) + edit.to;
    expectCount(sourcePath, text, end, 1);
    const until = text.indexOf(end);
    if (until < from) throw new Error(`opencode port: ${sourcePath}: cut ends before it starts`);
    return text.slice(0, from) + edit.to + text.slice(until);
  }
  const [from, to, count = 1] = edit;
  expectCount(sourcePath, text, from, count);
  return text.replaceAll(from, to);
}

/** `text` with every port edit for `sourcePath` applied; a source without a port is returned as is. */
export function applyPort(sourcePath: string, text: string): string {
  return (OPENCODE_PORTS[sourcePath] ?? []).reduce(
    (out, edit) => applyEdit(sourcePath, out, edit),
    text,
  );
}

const AGENT_TIER = /### Model tier\n\nThis agent runs on \*\*(Haiku|Sonnet|Opus)\*\*/g;

/** A toolu agent's model-tier paragraph, told how OpenCode picks its model. */
export function portAgent(surfaceId: string, sourcePath: string, text: string): string {
  const found = text.match(AGENT_TIER)?.length ?? 0;
  if (found !== 1) {
    throw new Error(
      `opencode port: ${sourcePath}: expected 1 model-tier paragraph, found ${found}`,
    );
  }
  return text.replace(
    AGENT_TIER,
    (_whole, tier: string) =>
      `### Model tier\n\nOn OpenCode this agent runs \`agent.${surfaceId}.model\` from your \`opencode.json\`, else the session's model. On Claude Code it runs on **${tier}**`,
  );
}
