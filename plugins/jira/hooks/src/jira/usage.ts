/** Help and setup text, byte-for-byte what the bash jira.sh printed to stderr. */

export const USAGE = `jira — Jira from the session (Cloud + Server/DC)

Usage: jira [--api-version N] [--lean] <family> <action> [options]

Families:
  search       JQL search
  issue        get|create|update|delete|comment|transition|transitions|assign
  board        list|get|issues
  sprint       list|get|create|issues|move|start|complete
  worklog      add|list|delete
  project      list|get|versions|components
  user         whoami|search|get
  attachment   add|list|get
  raw          <METHOD> <path> [body]
  plan         init|run|status|path

Environment:
  JIRA_BASE_URL                      required (e.g. https://acme.atlassian.net)
  JIRA_PAT                           Bearer auth (Cloud or Server/DC)
  JIRA_EMAIL + JIRA_API_TOKEN        basic auth (Cloud API token)
  JIRA_API_VERSION                   2 or 3 (default 3)
`;

export const CREDS_HELP = `jira: no Jira credentials found yet — let's get you connected.

This plugin reuses your \`jira\` CLI login automatically when it's configured,
or reads JIRA_* environment variables. Right now neither is set.

Connect with either:

  • Your jira CLI (easiest if it's installed):
      jira init
    then re-run your command — the plugin picks up the server, login, and
    API token from the CLI automatically.

  • Or environment variables:
      export JIRA_BASE_URL=https://your-site.atlassian.net
      export JIRA_EMAIL=you@example.com
      export JIRA_API_TOKEN=…        # id.atlassian.com → Security → API tokens
    (Jira Server/Data Center: use JIRA_PAT instead of JIRA_EMAIL + JIRA_API_TOKEN.)

Nothing is broken — this is just a one-time setup step.
`;

export const PLAN_USAGE = `jira plan — decompose ticket work into verifiable steps the dashboard renders

  plan init <KEY>                                 scaffold the plan doc for a ticket
  plan run <DOC> [--step <id>] [--activity <s>]   run checks, write the ledger
  plan status <KEY>                               print the ledger summary
  plan path <KEY>                                 print the ledger path

Every step's \`check\` must be a live Jira assertion that exits 0 only when Jira
itself reflects the change.
`;
