# Issue bodies

A sub-issue body is the prompt an unattended worker re-reads on every turn.
It is also what the engine routes on: a body over 1200 characters or with six
task boxes routes as `complex`, to a bigger model. Write what the worker needs
to build the right thing, and nothing else.

## Rules

1. **Cite or omit.** Name a file, symbol, field, constraint or behavior only
   with a `path:line` you read at the base commit. No citation, no claim.
2. **Decisions, not research.** State the approach, interface, files,
   non-goals and failure behavior. Alternatives and reasoning stay in the
   brainstorm record.
3. **One check per criterion.** Each acceptance criterion names the command or
   test that proves it on a real input. At most five; more means split.
4. **Front-load and stay small.** Keep the order below and stay under 1200
   characters. An interface too large for that goes in the epic once. Do not
   repeat the title or add a "Part of" header: the parent link carries both.
5. **Dependencies only under Relationships.** On any line, the reader takes
   every issue named after "blocked by" or "depends on" as a dependency, so use
   those phrases nowhere else.
6. **Links resolve.** Link a repository file only when
   `git cat-file -e "$SHA:<path>"` succeeds at the base commit; otherwise
   state the fact inline or drop it.
7. **One module each.** A child reads as Standard: one module, existing
   patterns, clear criteria. Split anything bigger along a dependency instead
   of adding caveats.

## Sub-issue

Title: a conventional-commit subject the PR can reuse.

```markdown
<One sentence: the observable change this PR makes.>

Base: <sha>

## Decisions
- <decision> (`<path>:<line>`)

## Interfaces
<signature, JSON shape or CLI flag; omit the section when there is none>

## Files
- `<path>`: <what changes>

## Acceptance criteria
- [ ] <observable outcome on a real input>. Check: `<command>`

## Non-goals
- <boundary>

## Relationships
Blocked by #<n>
```

Omit Relationships when the child has no blocker.

## Epic

Title `Epic: <name>`, label `epic`, under 4000 characters: every worker skims
it.

```markdown
<Goal in two sentences: the user-visible outcome and why now.>

Base: <sha>

## Decisions
- <one cited line per decision every child must honor>

## Conventions
Follow `AGENTS.md` (or `CLAUDE.md`) and the quality documents it links.

## Non-goals
- <boundary>
```

No task list: the native sub-issues carry the children. When native links are
missing, the reader takes every `- [ ] #N` line of the epic as a child.

## Example sub-issue

Title: `refactor(epic): let tests run the reader's text-blocker extraction`

```markdown
Let a unit test run the GitHub reader's text-blocker extraction without a network client.

Base: c817da76e

## Decisions
- Move the pattern loop of `text_blockers` (`crates/epic-orchestrator/src/tracker/github_fetch.rs:161`) into a pure function; `text_blockers` keeps the state fetch.
- Keep the pattern and its case-insensitive match (`crates/epic-orchestrator/src/tracker/github_fetch.rs:168`).

## Interfaces
`pub(super) fn text_blocker_refs(body: &str, repo: &str, own: &str) -> Result<Vec<String>, String>`

## Files
- `crates/epic-orchestrator/src/tracker/github_fetch.rs`: the extraction.

## Acceptance criteria
- [ ] A body naming one blocker twice and itself once yields that blocker once. Check: `cargo test -p toolu-epic-orchestrator --lib tracker::`
- [ ] The existing fetch fixtures pass unchanged. Check: the same target.

## Non-goals
- Changing which phrases count as a dependency.

## Relationships
Blocked by #12
```
