/**
 * OpenCode text for pr-babysit (#357). The shared workflow carries every host's
 * controller; OpenCode's copy keeps only its own, so no cron, goal, Claude Code
 * or Codex instruction reaches an OpenCode session. `cut` replaces the text from
 * one anchor up to (not including) another, or to the end when that is `null`;
 * each anchor must occur exactly once.
 */

export type PortEdit =
  | readonly [from: string, to: string, count?: number]
  | { readonly cut: readonly [start: string, end: string | null]; readonly to: string };

const SKILL = "plugins/pr-babysit/skills/babysit/SKILL.md";
const HELPER = "plugins/pr-babysit/skills/babysit/references/helper.md";
const WORKFLOW = "plugins/pr-babysit/workflows/babysit.md";

const SKILL_BODY = `This no-argument invocation explicitly authorizes babysitting the current
repository's pull request. A verified execution handoff is sufficient authorization
when execution already confirmed delivery authorization, GitHub
auth, a non-default branch, and this installed plugin; do not ask again or
introduce handoff arguments. Read [the canonical workflow](../../workflows/babysit.md)
completely and follow its OpenCode controller plus every shared strict-clearance
step.

Each check is one command, \`toolu babysit wait\` (or \`toolu babysit tick\` for a
check right now) with
\`--state-file "$REPO_ROOT/.opencode/tmp/pr-babysit/$SLOT.json"\`; its output
contract is [references/helper.md](references/helper.md). Trust that result:
never write a polling script or controller of your own, never re-fetch with
ad-hoc \`gh\` calls what the result already reports, and act through
\`toolu babysit reply\`, \`toolu babysit resolve\` and \`toolu babysit record\`.
Fix items go through \`toolu babysit route-fix --host opencode\` and, when it
dispatches fixer agents, \`toolu babysit dispatch-fix start\` then
\`toolu babysit dispatch-fix wait --timeout-seconds 45\` — one bounded wait per cycle.

OpenCode has no cron, goal or background command, so this turn is the
controller. After each cycle, run \`toolu babysit wait\` again (it returns within
55 s), until the Success or Escalation stop. Pending checks are neither
completion nor blockage. If the turn ends first, invoking this again resumes
from the state file.

Stop with success only after the same-cycle success audit proves CI green, zero
unresolved threads, and an approved zero-finding bot verdict; escalate only for a
genuine human-only blocker. For \`stop\` or \`cancel\`, run the workflow's OpenCode
cancel; cancellation is not completion.
`;

export const PR_BABYSIT_PORTS: Readonly<Record<string, readonly PortEdit[]>> = {
  [SKILL]: [
    ["explicitly asks Codex, or an authorized", "explicitly asks, or an authorized"],
    { cut: ["This no-argument invocation explicitly authorizes", null], to: SKILL_BODY },
  ],
  [HELPER]: [
    [
      "- `--state-file` — the host's slot path, verbatim: `/tmp/pr-babysit-<slot>.json`\n  (Claude), `<repo>/.codex/tmp/pr-babysit/<slot>.json` (Codex) or\n  `<repo>/.opencode/tmp/pr-babysit/<slot>.json` (OpenCode). Created on the\n  first tick, resumed after.",
      "- `--state-file` — the slot path, verbatim:\n  `<repo>/.opencode/tmp/pr-babysit/<slot>.json`. Created on the first tick,\n  resumed after.",
    ],
    [
      "- `--max-seconds` — the longest `wait` runs: 1800 on Claude (a background Bash\n  call), 55 on Codex, OpenCode and any other host.",
      "- `--max-seconds` — the longest `wait` runs: 55 s by default, under the bash\n  tool's timeout.",
    ],
    [
      "--items <file> --host claude|codex|opencode [--state-file <path>]",
      "--items <file> --host opencode [--state-file <path>]",
    ],
  ],
  [WORKFLOW]: [
    [
      "Route it (`--host` is this controller: `claude`, `codex` or `opencode`):",
      "Route it with `--host opencode`:",
    ],
    {
      cut: ["### Claude Code\n", "### OpenCode start or resume\n"],
      to: 'Slot: `SLOT="${OWNER}-${REPO}-${NUMBER}"` lowercased (e.g. `falconiere-toolu-42`).\n\n',
    },
    {
      cut: ["- **Single-slot scope.** Claude reads/writes only", "- **No cross-talk.**"],
      to: "- **Single-slot scope.** Read and write only\n  `$REPO_ROOT/.opencode/tmp/pr-babysit/$SLOT.json`. Never glob `*.json`, list the\n  state directory, or read another slot. The helper refuses a state file that\n  belongs to another PR (`slot_mismatch`) and a slot another controller holds\n  (`locked`, exit 75).\n",
    },
    {
      cut: ["- **Worktree isolation.**", "- **Stop is local.**"],
      to: "- **Worktree isolation.** Every code-change cycle uses its own worktree. Fixer\n  dispatch uses the slot's worktree on `pr-babysit/<slot>` (native\n  `<state>.worktree` when every group is OpenCode, else a herdr worktree),\n  recorded in state as `herdrWorktree`. Inline fixes use native `git worktree` at\n  `<state>.inline`. Never reuse another slot's worktree or fixer agent.\n",
    },
    [
      '--items "$PB_TMP/items.json" --host claude \\',
      '--items "$PB_TMP/items.json" --host opencode \\',
    ],
    {
      cut: [
        "| Fix looks like | Class | Claude Code | Codex | OpenCode |",
        "\nDeciding and doing are different classes",
      ],
      to: '| Fix looks like | Class | OpenCode |\n| --- | --- | --- |\n| One-line change, rename, typo, formatting, import, comment wording | `mechanical` | `task` with `subagent_type: "toolu-quick-task"` |\n| A bounded edit with a known answer plus its colocated test | `implementation` | `task` with `subagent_type: "toolu-implementer"` |\n| Cross-cutting, hard to reverse, several readings, needs weighing alternatives | `architecture` | `task` with `subagent_type: "toolu-architect"`, then implement |\n',
    },
    {
      cut: [
        "works only in the slot's herdr worktree (`toolu babysit dispatch-fix` owns it). Inline,",
        "\nReproduce + verify locally before push.",
      ],
      to: 'works only in the slot\'s fixer worktree (`toolu babysit dispatch-fix` owns it). Inline,\ncreate one detached worktree at exactly\n`$REPO_ROOT/.opencode/tmp/pr-babysit/$SLOT.inline`: validate the exact path, then\nrun `git worktree add --detach "$WORKTREE" "$HEAD_SHA"` (`HEAD_SHA` = `pr.head`\nfrom the result). It is inside the project, so neither you nor a `task` subagent\nmeets an `external_directory` prompt; give subagents absolute paths in it. Work on\ndetached HEAD and push with `git -C "$WORKTREE" push origin "HEAD:$BRANCH"`; this\navoids checking out a branch already held by the main checkout. Never reuse it\nfor a different PR.\n',
    },
    [
      "   the active host's `<worktree>/.claude/tmp/push-review/`,\n   `<worktree>/.codex/tmp/push-review/` or `<worktree>/.opencode/tmp/push-review/`\n   directory; push is denied otherwise.",
      "   `<worktree>/.opencode/tmp/push-review/`; push is denied otherwise.",
    ],
    {
      cut: [
        "`toolu babysit record status --status complete`, then Claude",
        '\n> "PR #N: all green',
      ],
      to: "`toolu babysit record status --status complete`, then remove the clean inline worktree, keep the state\nfile as the record, and end this turn's loop.",
    },
    [
      "Resume with `/pr-babysit:babysit` on Claude Code, `$pr-babysit:babysit` on Codex or the babysit command on OpenCode once unblocked.",
      "Resume with the babysit command once unblocked.",
    ],
    {
      cut: [
        "State is one exact file per slot: `/tmp/pr-babysit-${SLOT}.json` on Claude,",
        " The helper owns it",
      ],
      to: "State is one exact file per slot: `<repo>/.opencode/tmp/pr-babysit/${SLOT}.json`.",
    },
    [
      'wait --state-file "$STATE_FILE"   # Codex: --timeout-seconds 45',
      'wait --state-file "$STATE_FILE" --timeout-seconds 45',
    ],
    [
      "it fails. On Claude, run `wait` with the Bash tool's `timeout: 600000`; on\nCodex and OpenCode pass `--timeout-seconds 45`.",
      "it fails. Pass `--timeout-seconds 45`.",
    ],
    [
      "   Prefer the `toolu-review:review` workflow, which mirrors the CI bot and writes\n   compatible state. Claude may use its built-in code-review skill; Codex may\n   use its native review interface or a read-only review subagent. Apply",
      '   Prefer `skill({ name: "toolu-review-review" })`, which mirrors the CI bot and\n   writes compatible state. Apply',
    ],
    [
      "or inline the Claude host controls or Codex\n  and OpenCode native `git worktree` at the validated path recorded in this slot.",
      "or inline native `git worktree` at\n  the validated `<state>.inline` path.",
    ],
    [
      '"lastGoodSnapshot": "/tmp/pr-babysit-falconiere-toolu-42.snapshot.json"',
      '"lastGoodSnapshot": "<repo>/.opencode/tmp/pr-babysit/falconiere-toolu-42.snapshot.json"',
    ],
  ],
};
