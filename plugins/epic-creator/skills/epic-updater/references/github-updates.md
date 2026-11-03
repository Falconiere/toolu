# Safe GitHub body updates

Use only for edits authorized by the updater's scope. Keep the original JSON,
original body and desired body per issue in `$DIR`, plus a receipt recording
the issue URL, reviewed timestamp and each write's observed outcome.

Before the first write, re-read the parent, child membership, affected bodies,
comments, dependencies, PR state and engine status. If evidence changed since
the draft, reconcile the affected draft first. Immediately before each edit,
read that issue again and compare its body, state and `updated_at` to the
reviewed snapshot. A mismatch is a conflict, not permission to overwrite.
Preserve the concurrent edit and regenerate the affected diff. Obtain fresh
authorization only if the proposed action exceeds the existing scope.

Write only the body field, sequentially, from a file:

```bash
gh api "repos/$REPO/issues/$ISSUE" > "$DIR/$ISSUE-before-write.json"
gh api --method PATCH "repos/$REPO/issues/$ISSUE" -F body=@"$DIR/$ISSUE-after.md" > "$DIR/$ISSUE-response.json"
gh api "repos/$REPO/issues/$ISSUE" > "$DIR/$ISSUE-readback.json"
```

The first command is the conflict check's input: compare before executing
PATCH, not after it. Compare the readback body exactly with the file sent and
verify state and relationships separately. Do not send labels, assignees,
milestone or state alongside a body update. GitHub does not provide an atomic
multi-issue transaction here; preflight/readback narrow but cannot eliminate
the concurrent-write window. Defer an actively edited issue until coordinated.

On a timeout or nonzero response, stop further writes and read that issue:

- Desired body present: record the write as observed, do not repeat it.
- Original body present: report it as unapplied; revalidate before resuming.
- Different body or unreadable response: record conflict/unknown; preserve
  the drafts and do not retry or roll back blindly.

Report applied, unapplied and unknown issues with their URLs. Resume from
those receipts after revalidation, never replay the whole batch. Before a
repair or rollback, re-read and compare again so another person's work is
not lost. Native links and text relationships both affect the engine graph:
moving a phrase can change dependencies even when no link API was called.
