/** Keep the standalone Jira skill usable on OpenCode until issue #406 removes it. */
const OPENCODE_CONFIG = "${TOOLU_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/opencode}";

const JIRA_HOST_PAIR =
  /# Codex\nTOOLU_HOST_OVERRIDE=codex \\\n {2}"\$\{TOOLU_CONFIG_DIR:-\$\{CODEX_HOME:-\$HOME\/\.codex\}\}\/jira\/jira\.sh" (\[--api-version N\] \[--lean\] <family> <action> \[options\])\n# Claude Code\nTOOLU_HOST_OVERRIDE=claude \\\n {2}"[^"\n]*\/jira\/jira\.sh" \1/;
const JIRA_CHOOSE =
  /Choose the complete command for the active host, including its override; every\n`jira\.sh` shorthand below means that chosen prefix\. The override propagates to\nnested plan checks and their state paths\. Ordinary shell calls do not inherit\nplugin lifecycle variables, so never collapse these into one ambiguous\nfallback\. Use the published path; plugin-root variables are lifecycle-only\./;

/** Preserve the existing standalone skill's host-specific command while it is shipped. */
export function rewriteJiraSkill(skillId: string | undefined, text: string): string {
  if (skillId !== "jira-jira") return text;
  return text
    .replace(
      JIRA_HOST_PAIR,
      `# OpenCode\n"$TOOLU_BUN" --no-env-file "${OPENCODE_CONFIG}/jira/jira.sh" $1`,
    )
    .replace(
      JIRA_CHOOSE,
      '`shell.env` sets `TOOLU_BUN`, `TOOLU_CONFIG_DIR` and `TOOLU_HOST_OVERRIDE=opencode`\nin every bash call, so this runs with `bun` off `PATH` and never loads a project\n`.env`; every `jira.sh` shorthand below means this command. Plan checks call\n`"$JIRA"` with `.env` loading off too. A file of your own at that path runs\ndirectly instead. Plugin-root variables are lifecycle-only.',
    )
    .replace(
      "`.claude/tmp/jira/plans/<KEY>.md` or `.codex/tmp/jira/plans/<KEY>.md`.",
      "`.opencode/tmp/jira/plans/<KEY>.md`.",
    )
    .replace(
      "below the active host's `<repo>/.claude/tmp/plan-ledger/`\nor `<repo>/.codex/tmp/plan-ledger/`.",
      "below `<repo>/.opencode/tmp/plan-ledger/`.",
    );
}
