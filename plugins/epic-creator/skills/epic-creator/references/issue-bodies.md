# Issue bodies

A sub-issue body is the prompt an unattended worker re-reads on every turn.
It is also what the engine routes on: absent title/label overrides, an excerpt
over 1200 characters, six task boxes or fan-out of three raises the heuristic
tier. Jev can judge it differently. Keep the complete contract; brevity is a
cost target, not a reason to hide complexity or omit a failure case.

## Rules

1. **Cite or omit.** Name a file, symbol, field, constraint or behavior only
   with a `path:line` you read at the base commit. Label new files and
   interfaces as proposed; do not invent citations for them.
2. **Decisions, not research.** State the approach, interface, files,
   non-goals and failure behavior. Alternatives and reasoning stay in the
   brainstorm record.
3. **One check per criterion.** State the input and observable result, plus
   the focused command/test that proves it. Mark tests still to add as planned.
   Prefer at most five criteria; split only at a coherent delivery boundary.
4. **Front-load and stay small.** Keep the order below and target under 1200
   characters. A shared interface goes in the epic once. Do not
   repeat the title or add a "Part of" header: the parent link carries both.
5. **Dependencies only under Relationships.** On any line, the reader takes
   every issue named after "blocked by" or "depends on" as a dependency, so use
   those phrases nowhere else.
6. **Links resolve.** Link a repository file only when
   `git cat-file -e "$SHA:<path>"` succeeds at the base commit; otherwise
   state the fact inline or drop it.
7. **One coherent PR each.** Include callers, tests, fixtures and docs that
   must change together. Make sibling contracts and dependencies explicit.
   Explain necessary larger scope rather than forcing a Standard route.

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
- [ ] <input → observable outcome>. Check: `<command>` (<existing test or planned test to add>)

## Non-goals
- <boundary>

## Relationships
Blocked by #<n>
```

Omit Relationships when the child has no blocker.

## Epic

Title `Epic: <name>`, label `epic`, target under 4000 characters: every worker skims
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
