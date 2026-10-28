---
name: git-better
description: Write semantic commits and branch names that other agents can read without opening the diff — a scoped Conventional Commit subject plus an `Intent:` trailer, and `<type>/<ref>-<slug>` branches. Use to write a commit message, name a branch, or fix a commit git-better blocked; `toolu git-better check` validates first.
---

# git-better

The `semantic-git` rule checks every Bash command that commits or creates a
branch, before git runs. In `block` mode (the default) a message or name that
fails is denied with each failed check and a corrected example. Fix it and run
the command again; never work around the rule.

## Commit messages

```text
feat(git-better): block commits without an intent trailer

Optional body: what changed and anything a reviewer needs.

Intent: agents can tell why a commit exists without opening the diff
Risk: low
Refs: #740
```

- **Subject:** `type(scope): description`, at most 72 characters, no trailing
  period, and a lowercase imperative description (`add`, `fix`, `drop`). The
  type comes from `commit-prefixes.txt` (`feat`, `fix`, `docs`, `refactor`,
  `test`, `perf`, `build`, `ci`, `chore`, `style`, `revert`). The scope names
  the area that changed.
- **Body:** optional, after a blank line.
- **Trailers:** the last paragraph, one `Key: value` per line. Keep every
  trailer in that one paragraph, `Co-Authored-By:` included, with no blank
  line between them: git reads only the last paragraph.
  - `Intent:` is required. In one line, say why the change is needed: the
    problem it solves or the goal it serves, not what the diff does.
  - `Risk: low|medium|high` is optional.
  - `Refs:` is optional. Use it for issues and tickets (`#740`, `ENG-12`), and
    never put a ticket in front of the subject.
  - A breaking change needs `!` before the colon and a
    `BREAKING-CHANGE: <what breaks>` trailer, spelled with a hyphen. Git reads
    no trailers at all after a `BREAKING CHANGE:` line.
- **Always allowed:** `fixup!`, `squash!` and `amend!` subjects, `--amend
  --no-edit`, and `-C`. A `wip:` subject is allowed only off the base branch,
  and only when the branch will be squashed.

Pass the message so the rule can read it before the command runs: `-m` values,
a heredoc (`-m "$(cat <<'EOF' … EOF)"` or `-F - <<'EOF'`), a file that already
exists (`-F msg.txt`), or `--trailer 'Intent: …'`. The rule never guesses a
message it cannot read, such as `-m "$MSG"`, a pipe or the editor. It explains
instead, so check the message first:

```bash
toolu git-better check --message-file msg.txt
toolu git-better check -m "fix(cli): handle a missing flag" -m "Intent: the command crashed without it"
```

## Branch names

`<type>/[<ref>-]<slug>`: `feat/653-pause-and-resume-an-agent`,
`fix/ENG-12-retry-timeouts`, `docs/readme`. Use one `/`, an optional issue
number or tracker key, then a short lowercase kebab-case description. The rule
checks new names from `checkout -b/-B`, `switch -c/-C`, `branch <new>`,
`branch -m/-c` and `worktree add -b/-B`. Existing branches, the base branch and
`release-please--*` are exempt.

```bash
toolu git-better check --branch feat/740-git-better
```

## Provenance notes

After each successful commit, git-better attaches a JSON note under
`refs/notes/git-better`. It records the branch, base, host and session, and the
changed areas and files. Read it with
`git notes --ref git-better show <commit>`. Run `toolu git-better setup` once per
repository so amend and rebase carry the notes.

## Settings

`gitBetter` in `toolu.config.json`:
- `mode`: `block`, `advise` or `off`.
- `requireScope` and `requireRef`.
- `allowWip`.
- `types`.
- `notes` and `areaRoots`.

See `docs/config.md`.
