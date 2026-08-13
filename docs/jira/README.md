# jira — Jira Issue Search & Workflow

**Type:** Workflow | **Version:** 4.5.0 | **Standalone** (no dependencies)

Jira issue search and workflow from the session via a REST wrapper — a skill plus bash. Works with Jira Cloud and Server/Data Center, supporting both read and safe writes.

## Install

```text
/plugin install jira@toolu
```

## Setup

### Option 1 — Reuse jira CLI Config (Easiest)

If the [`jira` CLI](https://github.com/ankitpokhrel/jira-cli) is configured (`jira init`), the plugin reuses its login automatically — server + login from `~/.config/.jira/.config.yml`, API token from the OS keyring. **No extra setup.**

### Option 2 — Environment Variables

```bash
export JIRA_BASE_URL=https://acme.atlassian.net       # required (no trailing slash)

# Bearer auth — Personal Access Token (Cloud or Server/DC)
export JIRA_PAT=your-personal-access-token

# Basic auth — Cloud API token (used when JIRA_PAT is unset)
export JIRA_EMAIL=you@acme.com
export JIRA_API_TOKEN=your-api-token

# Server/Data Center → set API version to 2
export JIRA_API_VERSION=2
```

Resolution: explicit env wins; else the jira CLI config + keyring fill the gaps. Auth: `JIRA_PAT` → Bearer; else `JIRA_EMAIL`+`JIRA_API_TOKEN` → basic. When nothing is configured, the plugin prints a short setup prompt and exits.

## What It Provides

### `jira` Skill

Work a ticket without leaving the session: search, read, create, comment, transition, assign, manage sprints and worklogs, access attachments, and reach any endpoint via `raw`.

### `plan` Family

Decompose non-trivial ticket work into small, individually verifiable steps, tracked in a ledger.

## Plans

Read-only lookups (`issue get`, `search`, `board list`) run directly. Anything that **mutates** a ticket, or needs two or more calls, is planned first.

```bash
# Scaffold a plan doc, titled from a live `issue get`
jira.sh plan init ABC-123          # -> <repo>/<host-state>/tmp/jira/plans/ABC-123.md

# Author the steps, then run them
jira.sh plan run <printed-plan-path> --step transition-done
jira.sh plan run <printed-plan-path> --activity "closing out"

jira.sh plan status ABC-123        # jira-ABC-123  2/3 green   next: comment-pr-link
jira.sh plan path ABC-123          # ledger path; needs no credentials
```

Steps live in a `## Steps (machine-readable)` block:

````markdown
## Steps (machine-readable)

```json
[
  { "id": "transition-done",
    "title": "Move ABC-123 to Done",
    "check": "\"$JIRA\" issue get ABC-123 --lean | jq -e '.status==\"Done\"' >/dev/null" }
]
```
````

A `check` must exit 0 **only when Jira itself reflects the change** — `$JIRA` is bound to the CLI when it runs. Green means Jira agrees, not that the agent claimed success. Before running any check, `plan run` probes once with `user whoami`: if Jira is unreachable it aborts and writes **no** statuses, so an auth or network failure never marks a step red. A red step keeps the check's output in `evidence_tail`.

### The ledger

The ledger is written atomically to the native host path:
`<repo>/.claude/tmp/plan-ledger/jira-<KEY>.json` for Claude or
`<repo>/.codex/tmp/plan-ledger/jira-<KEY>.json` for Codex (schema version 1).

Two properties are deliberate:

- **It never blocks `git push`.** toolu's push gate reads only `<branch-slug>.json`; a file named for the issue key is invisible to it.
- **Jira steps never go stale.** The ledger records `base_branch: ""`, so a green Jira step stays green when unrelated code commits land.

## Usage Examples

### Search & Read Issues

```bash
# Search by JQL
jira.sh search -q 'project=PROJ AND status="In Progress"' --lean

# My open tickets
jira.sh search -q 'assignee=currentUser() AND resolution=Unresolved' -n 20

# Filter specific fields, strip noise
jira.sh search -q 'project=PROJ AND sprint in openSprints()' --fields summary,status,assignee --lean

# Read a specific issue
jira.sh issue get PROJ-1234 --lean
```

### Create & Manage Issues

```bash
# Create a new issue
jira.sh issue create -p PROJ -t Bug -s "Login page crashes on Safari" \
  -d "Steps to reproduce: 1. Open Safari 2. Navigate to /login 3. See white screen" \
  --assignee 557058:abc123

# Assign an issue
jira.sh issue assign PROJ-1234 557058:abc123

# Unassign (use '-' as account ID)
jira.sh issue assign PROJ-1234 -

# Update summary and description
jira.sh issue update PROJ-1234 -s "Updated: Login page crashes on Safari 17" -d "Updated reproduction steps..."

# Set a custom field
jira.sh issue update PROJ-1234 -f customfield_10042="high"
```

### Comment & Workflow Transitions

```bash
# Add a comment
jira.sh issue comment PROJ-1234 "Root cause identified: CSS grid regression in Safari 17. Fix in progress."

# List available transitions for an issue
jira.sh issue transitions PROJ-1234

# Transition to a specific status (name resolved case-insensitively)
jira.sh issue transition PROJ-1234 "In Review"
jira.sh issue transition PROJ-1234 31   # by transition ID
```

### Boards & Sprints

```bash
# List boards in a project
jira.sh board list -p PROJ

# Get board details
jira.sh board get 42

# Get board issues (optionally filtered by JQL)
jira.sh board issues 42 -q 'status != Done'

# List sprints on a board
jira.sh sprint list 42

# Get sprint details
jira.sh sprint get 101

# Get sprint issues
jira.sh sprint issues 101

# Create a sprint
jira.sh sprint create 42 -n "Sprint 14"

# Move issues to a sprint
jira.sh sprint move 101 PROJ-12 PROJ-34 PROJ-56

# Start a sprint
jira.sh sprint start 101

# Complete a sprint
jira.sh sprint complete 101
```

### Worklogs

```bash
# Log work on an issue
jira.sh worklog add PROJ-1234 -t 2h30m -c "Investigated Safari regression, identified CSS grid issue"

# List worklogs for an issue
jira.sh worklog list PROJ-1234

# Delete a worklog entry
jira.sh worklog delete PROJ-1234 12345
```

### Projects & Users

```bash
# List all projects
jira.sh project list

# Get project details
jira.sh project get PROJ

# List project versions
jira.sh project versions PROJ

# List project components
jira.sh project components PROJ

# Check current user
jira.sh user whoami

# Search for users
jira.sh user search -q "John Doe"

# Get user details
jira.sh user get 557058:abc123
```

### Attachments

```bash
# Attach a file to an issue
jira.sh attachment add PROJ-1234 ./screenshot.png

# List attachments on an issue
jira.sh attachment list PROJ-1234

# Download an attachment
jira.sh attachment download 12345 -o ./downloads/screenshot.png

# Read an attachment inline (text→context, binary→saved path)
jira.sh attachment read 12345
```

### Raw Endpoint Access

```bash
# Escape hatch — reach any endpoint the families don't cover
jira.sh raw GET /rest/api/3/field
jira.sh raw POST /rest/api/3/issue/bulk '{"issueUpdates": [...]}'
jira.sh raw DELETE /rest/api/3/version/10042
```

## Mutating Operations

These change live tickets and have **no undo prompt** (the verb is the confirmation):

- `issue create`, `issue update`, `issue comment`, `issue transition`, `issue assign`, `issue delete`
- `sprint create/move/start/complete`
- `worklog add/delete`
- `attachment add`

Always know the target `KEY`/`ID` before running them.

## CLI Options

| Flag | Effect |
|------|--------|
| `--api-version N` | Use API v2 (Server/DC) or v3 (Cloud, default) |
| `--lean` | Strip null/empty fields and verbose envelopes — cuts tokens when feeding to LLM |

## Notes & Constraints

| Topic | Detail |
|-------|--------|
| Search endpoint | v3 uses `POST /rest/api/3/search/jql` (nextPageToken); v2 uses `/rest/api/2/search` (startAt). `--all` follows whichever model applies. |
| Comment/description bodies | v3 needs ADF — the tool converts plain text automatically; v2 takes plain text. Rich ADF (tables, mentions) → use `raw`. |
| Boards & sprints | Always hit `/rest/agile/1.0` regardless of `--api-version`. |
| `-f field=val` | Sets string scalars only. Fields needing objects/arrays → use `jira raw`. |
| Anything unwrapped | `jira raw <METHOD> <path> [body]` reaches every endpoint the families don't cover. |
