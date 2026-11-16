# Size splits

A worker that cannot fit the PR size budget stops with a `needs-human` note:
`over size budget: <n> of <max>; kept on <branch>-rest; split: 1) … 2) …`.
Invoking this skill authorizes applying that split without asking the user
when all of these hold:

- The epic is on GitHub (`epic-creator:epic-updater` edits GitHub only).
- Every acceptance criterion of the issue lands in exactly one slice, and no
  slice adds a criterion. Write the mapping (criterion → slice) first.
- Each slice merges alone: its tests pass on the base plus the slices before it.

Anything else is a normal `needs-human`: ask the user.

## Apply

1. Run `epic-creator:epic-updater` on the epic with this split as its
   authorized change. It creates slices 2…n as new children through the
   creator's GitHub-write procedure, keeping the epic's membership format
   (a legacy task-list epic gets task-list entries, not native links). Each
   slice is blocked by the one before it; slice 2 by the split issue. Each new
   body keeps the issue's context, lists its own criteria and a `Size:`
   estimate, and names the prior work: `<branch>-rest`.
2. In the same run, narrow the split issue's body to slice 1's criteria, with
   one line naming where each moved criterion went.
3. Check the updater's after-graph: the new children, the chain of blockers,
   no cycle, and each original criterion in exactly one body.
4. Tell the worker:

   ```bash
   toolu epic answer <key> "NARROWED: your scope is slice 1; the issue body is updated. Slices 2… are #<n>, #<m>."
   ```

The engine needs no restart. A refresh reloads the epic's children: one runs
when a native epic's sub-issue list changes while any of its PRs waits, and
one runs after every merged issue's cleanup, before the next slice can be
unblocked. Never delete `<branch>-rest` or `refs/epic-wip/*`.
