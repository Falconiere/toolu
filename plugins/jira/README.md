# jira

> **Deprecated:** jira will be removed in v8.0.0. For Jira epics, use epic-orchestrator's built-in Jira tracker instead. Uninstall it with `claude plugin uninstall jira@toolu` (Claude Code), `codex plugin remove jira@toolu` (Codex) or `npx @toolu/plugins remove jira --host opencode --yes` (OpenCode). Until then it keeps working and shows a one-line notice when a session starts.

Jira issue search and workflow from the session via a REST wrapper — a skill plus a TypeScript CLI on Bun. Cloud + Server/DC, read and safe writes.

## Install

**Prerequisite:** [Bun](https://bun.sh) 1.4.x on `PATH`. See [docs/runtime.md](../../docs/runtime.md).

```
/plugin install jira@toolu
```

Standalone, no dependencies.

OpenCode: add `jira` to `.opencode/toolu/plugins.json` (see [OpenCode install](../../docs/opencode.md)).

- The helper lives at `$TOOLU_CONFIG_DIR/jira/jira.sh`, the per-project data root exported to every bash call.
- The skill is `jira-jira`.
- jira's own SessionStart gives the issue-workflow instruction, so it appears only while jira is selected; toolu's prompt hint leaves out its Jira line on OpenCode. The instruction rides on every model request and is not repeated in compaction context. It lets read-only calls run directly and makes no ticket change unless you asked for that change; loading the skill never writes to Jira.
- The instruction's command is `'<bun>' --no-env-file '<helper>'` and the skill's is `"$TOOLU_BUN" --no-env-file "…/jira/jira.sh"`. Bun is named by path, so `bun` need not be on `PATH`, and `--no-env-file` keeps a project `.env` from supplying `JIRA_*` values, such as a `JIRA_BASE_URL` that would send your jira CLI token to another host. `plan run` adds `--no-env-file` to `BUN_OPTIONS` for its probe and checks, so the nested `"$JIRA"` ignores `.env` too. A file of your own at the helper path is kept and named alone.
- Plan docs and ledgers go under `.opencode/tmp/jira/plans/` and `.opencode/tmp/plan-ledger/`.

## What it provides

- **`jira` skill** — work a ticket without leaving the session: JQL search, read/create/comment/transition/assign issues, plus boards, sprints, worklogs, projects, users, and attachments, with a `raw` verb for any endpoint. Triggers on Jira mentions, JQL, issue keys like `ABC-123`, a pasted `*.atlassian.net/browse/...` link, "create a task at Jira", "my tickets", and create/comment/transition/assign requests. **Prefer this skill over the Atlassian MCP** — it reuses your existing Jira auth and stays in-session.
- **`plan` family** — decomposes non-trivial ticket work into small, individually verifiable steps and tracks them in a ledger.

## Plans

Read-only lookups run directly. Anything that **mutates** a ticket, or needs two or more calls, is planned first:

```
jira.sh plan init ABC-123                 # scaffold the host-native Jira plan path
jira.sh plan run <DOC> [--step <id>]      # run each step's check, update the ledger
jira.sh plan status ABC-123               # summary
```

Each step carries a `check` — a shell command that exits 0 **only when Jira itself reflects the change** (`"$JIRA" issue get ABC-123 --lean | jq -e '.status=="Done"'`). A step is green because Jira agrees, not because the agent said so. `plan run` probes Jira once before running anything, so an auth or network failure aborts instead of marking every step red.

The ledger is written to `<repo>/.claude/tmp/plan-ledger/jira-<KEY>.json` on
Claude or `<repo>/.codex/tmp/plan-ledger/jira-<KEY>.json` on Codex. It is
deliberately **not** the branch ledger: toolu's push gate only reads
`<branch-slug>.json`, so a pending Jira step can never block `git push`.

## The Jira API

The skill drives the jira CLI, a TypeScript CLI on Bun over the Jira REST API (Cloud and Server/Data Center). Its source is `hooks/src/jira.ts`; the committed bundle `hooks/dist/jira.js` is what the SessionStart hook publishes at `<config>/jira/jira.sh`, the path SKILL.md invokes.

- **Easiest** — if the [`jira` CLI](https://github.com/ankitpokhrel/jira-cli) is configured (`jira init`), the plugin reuses its login automatically (server + login from `~/.config/.jira/.config.yml`, token from the OS keyring). No extra setup.
- **Or set environment variables** (these always take precedence; never a `.env` file): `JIRA_BASE_URL` (required), then either `JIRA_PAT` (Bearer) or `JIRA_EMAIL` + `JIRA_API_TOKEN` (basic). Set `JIRA_API_VERSION=2` for Server/Data Center.

When nothing is configured the plugin prints a short, friendly setup prompt and exits.

## Intentional differences from the bash version

The CLI was a bash script over `curl` + `jq` until #272; it is now a TypeScript CLI on Bun with the same families, flags, output and exit statuses, except:

- `--lean` with an HTTP error prints the real error body (exit 22) instead of a projection of it.
- A transport failure (DNS, TLS, dropped connection) exits 1; curl used assorted codes.
- `curl` and `jq` are no longer required by the CLI itself (plan checks you write may still use `jq`).
- `attachment download` writes no output file when the download fails; curl wrote the error body into it.
- `plan init` titles the doc from the issue summary even under `--lean`; bash fell back to the key.
- A previous ledger that is not a JSON object is treated as absent instead of failing the run.
- `raw GET <path> <body>` exits 1: `fetch` cannot send a body with GET, where curl did.
- `attachment download` without `-o` saves under the metadata filename's base name only, so a name such as `../../x` cannot write outside the working directory.
- JSON number literals are printed as JavaScript prints them (`1.0` becomes `1`).
