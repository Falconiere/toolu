# Child detail request measurement

Issue #760, 2026-10-30. Workload: a release-profile focused tracker test replays
the captured GitHub native sub-issue #760 response from
`fixtures/epic/native-sub-issue-760.json` through the real loopback HTTPS client.
The response is a sanitized native list item: only fields read by the tracker
were retained. The dependency response and closing-PR response were empty;
the live closing-PR query for #760 also returned no nodes. Platform: macOS,
Apple Silicon. Sample count: one baseline release test and one changed release
test; these are deterministic request counts, not latency benchmarks.

| Version | Command | Native list GET | Child detail GET | Dependency GET | GraphQL POST | Total |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Before | `cargo nextest run --release -p toolu-epic-orchestrator -E 'test(tracker::github_fetch::tests::)' --nocapture` | 1 | 1 | 1 | 1 | 4 |
| After | `cargo nextest run --release -p toolu-epic-orchestrator -E 'test(tracker::github_fetch::)' --nocapture` | 1 | 0 | 1 | 1 | 3 |

The baseline test failed at its new request-count assertion (`left: 1`,
`right: 0`), establishing the extra GET. The changed release test passed with
zero child detail GETs and three total requests. The task-list loopback test
also counts one initial issue GET per distinct child (seven total requests for
two children). Omitted and null native fields each restore exactly one child
detail GET. Existing `map_limit` coverage confirms no more than four concurrent
workers. This change does not alter the dependency or GraphQL calls.
