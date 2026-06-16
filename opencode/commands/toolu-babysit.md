---
description: Babysit a PR — fetch unresolved comments + CI review-bot verdict, triage, fix, reply, resolve, chase findings to zero.
agent: build
---

# Babysit a PR

Babysit the PR for the current branch. Each tick: fetch unresolved comments **and the CI review-bot verdict** → triage → fix → reply → resolve. CI fails → fix + re-push. Stop only when **no unresolved comments, the bot verdict has zero findings and is approved, AND CI all green**.

## Inputs

- **no args** _(default)_ — babysit PR for current branch in CWD.
- **`stop`** — cancel this slot's cron + clear state. Nothing else runs.

No other flags. Don't add any. Want different behavior → edit this file.

## Target resolution

Target = PR for current branch:

```bash
REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"
BRANCH=$(git branch --show-current)
PR_JSON=$(gh pr list --head "$BRANCH" --json number,url,headRepository --jq '.[0]')
```

Extract `number`, `owner` (`headRepository.owner.login`), `repo` (`headRepository.name`), `PR_AUTHOR` (skip self-replies):

```bash
PR_AUTHOR=$(gh pr view "$NUMBER" --json author --jq '.author.login')
```

No PR for branch → report + exit.
