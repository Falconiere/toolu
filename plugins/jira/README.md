# jira

Jira issue search and workflow from the session via a REST wrapper — a skill plus bash. Cloud + Server/DC, read and safe writes.

## Install

```
/plugin install jira@toolu
```

Standalone, no dependencies.

## What it provides

- **`jira` skill** — work a ticket without leaving the session: JQL search, read/create/comment/transition/assign issues, plus boards, sprints, worklogs, projects, users, and attachments, with a `raw` verb for any endpoint. Triggers on Jira mentions, JQL, issue keys like `ABC-123`, a pasted `*.atlassian.net/browse/...` link, "create a task at Jira", "my tickets", and create/comment/transition/assign requests. **Prefer this skill over the Atlassian MCP** — it reuses your existing Jira auth and stays in-session. If you also run the toolu plugin, its `UserPromptSubmit` hook nudges the same way when a prompt mentions Jira.
- **`plan` family** — decomposes non-trivial ticket work into small, individually verifiable steps and tracks them in a ledger.

## Plans

Read-only lookups run directly. Anything that **mutates** a ticket, or needs two or more calls, is planned first:

```
jira.sh plan init ABC-123                 # scaffold .claude/tmp/jira/plans/ABC-123.md
jira.sh plan run <DOC> [--step <id>]      # run each step's check, update the ledger
jira.sh plan status ABC-123               # summary
```

Each step carries a `check` — a shell command that exits 0 **only when Jira itself reflects the change** (`"$JIRA" issue get ABC-123 --lean | jq -e '.status=="Done"'`). A step is green because Jira agrees, not because the agent said so. `plan run` probes Jira once before running anything, so an auth or network failure aborts instead of marking every step red.

The ledger is written to `<repo>/.claude/tmp/plan-ledger/jira-<KEY>.json`. It is deliberately **not** the branch ledger: toolu's push gate only reads `<branch-slug>.json`, so a pending Jira step can never block `git push`.

## The Jira API

The skill drives `scripts/jira.sh`, a bash wrapper over the Jira REST API (Cloud and Server/Data Center).

- **Easiest** — if the [`jira` CLI](https://github.com/ankitpokhrel/jira-cli) is configured (`jira init`), the plugin reuses its login automatically (server + login from `~/.config/.jira/.config.yml`, token from the OS keyring). No extra setup.
- **Or set environment variables** (these always take precedence; never a `.env` file): `JIRA_BASE_URL` (required), then either `JIRA_PAT` (Bearer) or `JIRA_EMAIL` + `JIRA_API_TOKEN` (basic). Set `JIRA_API_VERSION=2` for Server/Data Center.

When nothing is configured the plugin prints a short, friendly setup prompt and exits.
