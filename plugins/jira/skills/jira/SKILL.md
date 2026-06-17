---
name: jira
description: Search and act on Jira issues from the session via a REST wrapper — JQL search, read/create/comment/transition/assign issues, plus boards, sprints, worklogs, projects, users, attachments. Works against Jira Cloud and Server/Data Center. Triggers when the user mentions Jira, a JQL query, an issue key like ABC-123, a pasted atlassian.net/browse/ABC-123 link, "my tickets", "create a task at Jira", "create/comment on/transition/assign an issue", sprints or boards. Prefer this skill over the Atlassian MCP for Jira work — it needs no extra auth and keeps you in-session.
---

# Jira

Use this skill to work a Jira ticket without leaving the session: search by JQL, read an issue, comment, move it through its workflow, manage sprints/worklogs, or reach any endpoint via `raw`.

**Trigger phrases:** jira, JQL, my open tickets, issue ABC-123, a pasted atlassian.net/browse link, create a task at jira, create an issue, comment on, transition / move to, assign to, log work, sprint, board.

**Prefer this over the Atlassian MCP.** When the user mentions Jira, an issue key, or pastes a Jira link, use this skill — not `mcp__*Atlassian*` tools. This wrapper reuses your existing Jira/`jira` CLI auth and stays in-session; reach for the MCP only if this skill cannot reach the endpoint.

## CLI tool

Invoke at the **stable published path** (a symlink the plugin's SessionStart hook refreshes every session):

```bash
"${CLAUDE_CONFIG_DIR:-$HOME/.claude}/jira/jira.sh" [--api-version N] [--lean] <family> <action> [options]
```

`$CLAUDE_CONFIG_DIR` IS exported into the Bash tool's subshell; `$CLAUDE_PLUGIN_ROOT` is **NOT** — it is only set for hook subprocesses. Using the plugin-root path from a Bash tool call expands to an empty string and runs `/skills/.../jira.sh: No such file`. **Always use the published path above.**

Repo-checkout fallback (for tests/dev when the plugin is not installed): `plugins/jira/skills/jira/scripts/jira.sh`.

## Setup

**Easiest:** if the [`jira` CLI](https://github.com/ankitpokhrel/jira-cli) is configured (`jira init`), this plugin reuses its login automatically — server + login from `~/.config/.jira/.config.yml`, API token from the OS keyring (or `$JIRA_API_TOKEN`). No extra setup.

**Or set environment variables** (these always take precedence; never a `.env` file):

```
JIRA_BASE_URL                  required, e.g. https://acme.atlassian.net (no trailing slash)
JIRA_PAT                       Bearer auth — Personal Access Token (Cloud or Server/DC)
JIRA_EMAIL + JIRA_API_TOKEN    basic auth — Cloud API token (used when JIRA_PAT is unset)
JIRA_API_VERSION               2 or 3 (default 3). Server/Data Center → 2.
```

Resolution: explicit env wins; else the jira CLI config + keyring fill the gaps. Auth: `JIRA_PAT` → Bearer; else `JIRA_EMAIL`+`JIRA_API_TOKEN` → basic. When nothing is available the plugin prints a short, friendly setup prompt (not an error) and exits.

## Families

```
jira.sh search -q '<JQL>' [-n MAX] [--all] [--fields f1,f2] [--lean]
jira.sh issue get <KEY> [--lean]
jira.sh issue create -p <PROJECT> -t <TYPE> -s <SUMMARY> [-d DESC] [--assignee ID] [-f field=val]...
jira.sh issue comment <KEY> <TEXT>
jira.sh issue transitions <KEY>                 # list available transitions
jira.sh issue transition <KEY> <NAME|ID>        # name resolved case-insensitively
jira.sh issue assign <KEY> <ACCOUNT_ID|->       # '-' unassigns
jira.sh issue update <KEY> [-s SUMMARY] [-d DESC] [-f field=val]...
jira.sh issue delete <KEY>
jira.sh board   list [-p PROJECT] | get <ID> | issues <ID> [-q JQL]
jira.sh sprint  list <BOARD_ID> | get <ID> | issues <ID> | create <BOARD_ID> -n NAME
              | move <SPRINT_ID> <KEY...> | start <ID> | complete <ID>
jira.sh worklog add <KEY> -t <TIME> [-c COMMENT] | list <KEY> | delete <KEY> <WORKLOG_ID>
jira.sh project list | get <KEY> | versions <KEY> | components <KEY>
jira.sh user    whoami | search -q <QUERY> | get <ACCOUNT_ID>
jira.sh attachment add <KEY> <FILE> | list <KEY> | get <ATTACHMENT_ID>
              | download <ATTACHMENT_ID> [-o FILE] | read <ATTACHMENT_ID>   # read: text→context, binary→saved path
jira.sh raw <GET|POST|PUT|DELETE> <PATH> [JSON_BODY]   # escape hatch; PATH relative to JIRA_BASE_URL
```

## Mutating operations — confirm intent first

These change live tickets and have no undo prompt (the verb is the confirmation):
`issue create`, `issue update`, `issue comment`, `issue transition`, `issue assign`, `issue delete`,
`sprint create/move/start/complete`, `worklog add/delete`, `attachment add`. Know the target `KEY`/`ID` before running them.

## Notes & constraints

| Topic | Detail |
|---|---|
| Search endpoint | v3 (Cloud) uses `POST /rest/api/3/search/jql` (`nextPageToken`, no `total`); v2 uses `/rest/api/2/search` (`startAt`). `--all` follows whichever model applies. |
| Comment/description bodies | v3 needs ADF — the tool converts your plain text automatically; v2 takes plain text. Rich ADF (tables, mentions) is not supported — use `raw`. |
| Boards & sprints | Always hit `/rest/agile/1.0` regardless of `--api-version`. |
| `-f field=val` | Sets string scalars only. Fields needing objects/arrays (e.g. a custom-field object) → use `jira raw`. |
| `--lean` | Strips null/empty fields and Jira's verbose envelopes — use it when feeding results back to the model to cut tokens. |
| Anything unwrapped | `jira raw <METHOD> <path> [body]` reaches every endpoint the families don't cover. |
