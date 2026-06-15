# jira

Jira issue search and workflow from the session via a REST wrapper — a skill plus bash. Cloud + Server/DC, read and safe writes.

## Install

```
/plugin install jira@toolu
```

Standalone, no dependencies.

## What it provides

- **`jira` skill** — work a ticket without leaving the session: JQL search, read/create/comment/transition/assign issues, plus boards, sprints, worklogs, projects, users, and attachments, with a `raw` verb for any endpoint. Triggers on Jira mentions, JQL, issue keys like `ABC-123`, "my tickets", and create/comment/transition/assign requests.

## The Jira API

The skill drives `scripts/jira.sh`, a bash wrapper over the Jira REST API (Cloud and Server/Data Center).

- **Easiest** — if the [`jira` CLI](https://github.com/ankitpokhrel/jira-cli) is configured (`jira init`), the plugin reuses its login automatically (server + login from `~/.config/.jira/.config.yml`, token from the OS keyring). No extra setup.
- **Or set environment variables** (these always take precedence; never a `.env` file): `JIRA_BASE_URL` (required), then either `JIRA_PAT` (Bearer) or `JIRA_EMAIL` + `JIRA_API_TOKEN` (basic). Set `JIRA_API_VERSION=2` for Server/Data Center.

When nothing is configured the plugin prints a short, friendly setup prompt and exits.
