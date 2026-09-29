# Shell analysis fixtures

Real command lines for `@toolu/core/shell` (#284). The tests live in
`packages/toolu-core/src/shell/__tests__/`.

- `bats-parity.json` holds every distinct input that the shipped bash functions
  received while the plugin bats suites ran. The functions are `is_git_push`,
  `is_git_commit`, `bash_write_targets`, `bash_commands_decide`,
  `push_target_root` and `push_target_branch`, and each case lists the suites it
  came from. `bash` is the unmodified bash result. `bats-parity.test.ts`
  requires the TypeScript result to equal it, or `expected` where a case notes a
  documented bash over-inclusion. `push_target_root` cases name the git repos to
  create (`repos`), the working directory and any environment. `$TMP` and
  `$MKTEMP` stand for directories in a fresh temp layout.
- `issue-283.json` holds one named fixture per example in
  [#283](https://github.com/Falconiere/toolu/issues/283). `expected` is the
  correct result and `bash` the known-wrong baseline. `oracle: "live"` baselines
  are re-derived from bash on every run. `oracle: "recorded"` baselines came from
  shipped hook scripts (commit-gate, gate-status, search-nudge) or wall-clock
  numbers, and #283 reproduced them on v7.2.0.

`bash-oracle.test.ts` sources `plugins/toolu/hooks/lib/detect.sh` and
`pre-tools/modules/bash-commands.sh` unmodified and re-derives every live `bash`
value. When the bash implementation is deleted (#279), delete that test. The
fixtures stay as the record of what the TypeScript layer must keep answering.

**How the inputs were harvested.** A `BASH_ENV` probe composed a DEBUG-trap
snippet onto every bash process the suites started, including bats' own `run`
frame. It logged the first argument each of the six functions received, and
wrote only to a temp log. Harvesting instrumented nothing under `plugins/`.
