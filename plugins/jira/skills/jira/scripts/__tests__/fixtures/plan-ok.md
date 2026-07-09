# ABC-123 — ship the release notes

**Date:** 2026-07-09   **Issue:** ABC-123   **Topic:** move the ticket to Done and record the PR link

## Steps (machine-readable)

```json
[
  {
    "id": "comment-pr-link",
    "title": "Comment the PR link on ABC-123",
    "check": "\"$JIRA\" issue get ABC-123 --lean | jq -e '.key==\"ABC-123\"' >/dev/null"
  },
  {
    "id": "transition-done",
    "title": "Move ABC-123 to Done",
    "check": "\"$JIRA\" issue get ABC-123 --lean | jq -e '.status==\"Done\"' >/dev/null",
    "activity": "transitioning"
  }
]
```

## Notes

Prose after the block must not be captured by the parser.
