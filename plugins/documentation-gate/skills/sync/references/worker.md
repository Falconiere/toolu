# Documentation worker

Audit the committed diff, implementation, relevant tests and existing documentation. Fix stale documentation and add missing explanations, using real behavior and runnable checks. Do not commit, push, delegate, or audit unrelated history. Return edits, observed checks, and unresolved findings to the parent.

After the parent commits, review the final committed diff again and return one JSON report built from the fresh `toolu docs inspect` identity: `version`, `repo`, `branch`, `base_branch`, `diff_sha`, and `reviewed_files` unchanged. Add the exact Jev `route` (`source`, `candidate_id`, `host`, `model`, `effort`) and `worker` (`id`, `host`, `model`, `effort`) identifying this actual worker. The worker tuple must match the route.

Add `coverage` with exactly one row per reviewed file: `path`, `disposition` (`updated`, `accurate`, or `not-needed`), substantive `note`, and `docs` array. Updated and accurate rows require committed repository documentation paths; updated must reference a changed documentation file. Include nonempty `checks` strings describing observed commands and results, and `findings` as an empty array only when no findings remain. Report unresolved findings honestly; they prevent recording. The writer validates structure and freshness, not the semantic truth of the review.
