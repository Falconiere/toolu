#!/usr/bin/env bats
# Real-data tests for scripts/collect-pr.sh and the page-merge programs in
# scripts/lib/normalize.sh.
#
# Offline block: captured `gh api --paginate --slurp` pages from
# Falconiere/toolu#115 and #165 (page size 2, captured 2026-09-19) and the
# real `gh` binary pointed at a refused loopback port.
# Live block (PR_BABYSIT_LIVE=1 + gh auth): the same merged PRs, comparing a
# page-size-2 collection against a default-page-size collection and against
# direct gh counts. No mocks anywhere.

SCRIPTS="${BATS_TEST_DIRNAME}/.."
FX="${BATS_TEST_DIRNAME}/fixtures/gh"
SNAP="${BATS_TEST_DIRNAME}/fixtures/snapshots"

setup() {
  TMP=$(mktemp -d)
  command -v gh >/dev/null 2>&1 || skip "gh not installed"
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

with_norm() {
  bash -c "set -euo pipefail; . '$SCRIPTS/lib/normalize.sh'; $1"
}

live() {
  [ "${PR_BABYSIT_LIVE:-}" = 1 ] || skip "set PR_BABYSIT_LIVE=1 to run against api.github.com"
  gh auth status >/dev/null 2>&1 || skip "gh not authenticated"
}

# --- normalize.sh page merges (captured real pages) -------------------------

@test "normalize: 9 captured reviewThreads pages merge into 17 threads with the snapshot thread shape" {
  out=$(with_norm "jq \"\$(pb_jq_threads_from_pages)\" '$FX/pages-115-threads-p2.json'")
  [ "$(jq 'length' <<<"$out")" -eq 17 ]
  [ "$(jq -r '.[0] | keys | join(",")' <<<"$out")" = "comments,commentsEndCursor,commentsHasNextPage,id,isOutdated,isResolved,line,path" ]
  [ "$(jq -r '.[0].comments[0] | keys | join(",")' <<<"$out")" = "author,authorType,body,createdAt,databaseId,id,url" ]
  [ "$(jq -r '[.[].comments[].authorType] | unique | join(",")' <<<"$out")" = "Bot,User" ]
  # GraphQL form of the CI reviewer login (no [bot] suffix) survives as-is.
  [ "$(jq '[.[].comments[].author] | index("github-actions") != null' <<<"$out")" = true ]
  [ "$(jq '[.[] | select(.isOutdated)] | length' <<<"$out")" -eq 1 ]
}

@test "normalize: 2 captured node(id:) comment pages merge into 3 comments in order" {
  out=$(with_norm "jq \"\$(pb_jq_thread_comments_from_pages)\" '$FX/pages-165-thread-comments-p2.json'")
  [ "$(jq 'length' <<<"$out")" -eq 3 ]
  [ "$(jq -r 'map(.author) | join(",")' <<<"$out")" = "github-actions,Falconiere,github-actions" ]
  [ "$(jq '[.[].databaseId] | all(type == "number")' <<<"$out")" = true ]
}

@test "normalize: REST pages flatten — 10 review pages → 20 reviews, 2 comment pages → 3 comments" {
  [ "$(with_norm "jq \"\$(pb_jq_rest_items) | map(\$(pb_jq_review)) | length\" '$FX/pages-115-reviews-p2.json'")" -eq 20 ]
  out=$(with_norm "jq \"\$(pb_jq_rest_items) | map(\$(pb_jq_issue_comment))\" '$FX/pages-115-comments-p2.json'")
  [ "$(jq 'length' <<<"$out")" -eq 3 ]
  [ "$(jq -r '.[0].author' <<<"$out")" = "github-actions[bot]" ]
  [ "$(jq -r '.[0] | keys | join(",")' <<<"$out")" = "author,authorType,body,createdAt,id,updatedAt,url" ]
  [ "$(jq -r '.[0].authorType' <<<"$out")" = Bot ]
}

@test "normalize: is_ci_reviewer matches both login forms exactly and never by [bot] substring" {
  for l in github-actions 'github-actions[bot]' claude 'claude[bot]'; do
    [ "$(with_norm "jq -n --arg l '$l' \"\$(pb_jq_defs) \\\$l | is_ci_reviewer\"")" = true ]
  done
  for l in dependabot 'dependabot[bot]' Falconiere 'renovate[bot]'; do
    [ "$(with_norm "jq -n --arg l '$l' \"\$(pb_jq_defs) \\\$l | is_ci_reviewer\"")" = false ]
  done
}

@test "normalize: check_state / ci_status over the captured PR#115 rollup and boundary shapes" {
  rollup=$(jq -c '.pr.statusCheckRollup' "$SNAP/toolu-115.json")
  [ "$(with_norm "jq -c \"\$(pb_jq_defs) ci_status\" <<<'$rollup'")" = '"pass"' ]
  [ "$(with_norm "jq -c \"\$(pb_jq_defs) ci_status\" <<<'[]'")" = '"pending"' ]
  mixed='[{"__typename":"CheckRun","name":"a","status":"COMPLETED","conclusion":"SUCCESS"},{"__typename":"StatusContext","context":"b","state":"PENDING"}]'
  [ "$(with_norm "jq -c \"\$(pb_jq_defs) ci_status\" <<<'$mixed'")" = '"pending"' ]
  failing='[{"__typename":"CheckRun","name":"a","status":"IN_PROGRESS","conclusion":null},{"__typename":"StatusContext","context":"b","state":"FAILURE"}]'
  [ "$(with_norm "jq -c \"\$(pb_jq_defs) ci_status\" <<<'$failing'")" = '"fail"' ]
  soft='[{"__typename":"CheckRun","name":"a","status":"COMPLETED","conclusion":"NEUTRAL"},{"__typename":"CheckRun","name":"b","status":"COMPLETED","conclusion":"SKIPPED"},{"__typename":"StatusContext","context":"c","state":"SUCCESS"}]'
  [ "$(with_norm "jq -c \"\$(pb_jq_defs) ci_status\" <<<'$soft'")" = '"pass"' ]
}

# --- collect-pr.sh: usage and transport failure (offline) --------------------

@test "collect-pr: usage errors exit 2 with a structured error" {
  run bash "$SCRIPTS/collect-pr.sh" --repo x --pr 1 --out "$TMP/s.json"
  [ "$status" -eq 2 ]; [ "$(jq -r '.errors[0].code' <<<"$output")" = usage ]
  run bash "$SCRIPTS/collect-pr.sh" --repo x/y --pr one --out "$TMP/s.json"
  [ "$status" -eq 2 ]
  run bash "$SCRIPTS/collect-pr.sh" --repo x/y --pr 1 --out "$TMP/s.json" --page-size 0
  [ "$status" -eq 2 ]
  run bash "$SCRIPTS/collect-pr.sh" --bogus
  [ "$status" -eq 2 ]
  [ ! -e "$TMP/s.json" ]
}

@test "collect-pr: refused host → exit 3 api_error after 3 attempts, no snapshot, bounded (AC-12)" {
  start=$(date +%s)
  run env GH_HOST=127.0.0.1:1 GH_TOKEN=x PB_GH_BACKOFF='0 0 0' bash "$SCRIPTS/collect-pr.sh" --repo x/y --pr 1 --out "$TMP/s.json"
  end=$(date +%s)
  [ "$status" -eq 3 ]
  doc=$(printf '%s\n' "$output" | grep '^{')
  [ "$(jq -r '.errors[0].code' <<<"$doc")" = api_error ]
  [ "$(jq -r '.errors[0].attempts' <<<"$doc")" = 3 ]
  [ "$(jq -r '.errors[0].class' <<<"$doc")" = transient ]
  [[ "$(jq -r '.errors[0].lastMessage' <<<"$doc")" == *"connection refused"* ]]
  [ ! -e "$TMP/s.json" ]
  [ $((end - start)) -lt 60 ]
  # No scratch dir or temp file left behind.
  [ -z "$(ls -A "$TMP")" ]
}

# --- captured snapshots are what the collector produces ---------------------

@test "snapshot fixtures: toolu-115 (page size 2) carries the full PR at 17/3/20 with pagination counts" {
  f="$SNAP/toolu-115.json"
  [ "$(jq -r .version "$f")" = 1 ]
  [ "$(jq -r .repo "$f")" = Falconiere/toolu ]
  [ "$(jq -r .number "$f")" = 115 ]
  [ "$(jq -r .pageSize "$f")" = 2 ]
  [ "$(jq -r '.head.verifiedAfterFanout' "$f")" = true ]
  [ "$(jq -r '.head.sha' "$f")" = "$(jq -r '.pr.headRefOid' "$f")" ]
  [ "$(jq '.threads | length' "$f")" -eq 17 ]
  [ "$(jq '.comments | length' "$f")" -eq 3 ]
  [ "$(jq '.reviews | length' "$f")" -eq 20 ]
  [ "$(jq '.pages.threads' "$f")" -ge 9 ]
  [ "$(jq '.pages.comments' "$f")" -ge 2 ]
  [ "$(jq '.pages.reviews' "$f")" -ge 10 ]
  [ "$(jq -r '.bot.comment.author' "$f")" = "github-actions[bot]" ]
  [ "$(jq -r '.bot.verdict.is_review_comment' "$f")" = true ]
  [ "$(jq -r '.bot.verdict.state' "$f")" = provider_error ]
}

@test "snapshot fixtures: toolu-165 (page size 2) completes a 3-comment thread through the node(id:) cursor loop" {
  f="$SNAP/toolu-165.json"
  [ "$(jq '.threads | length' "$f")" -eq 6 ]
  [ "$(jq '[.threads[].comments | length] | max' "$f")" -eq 3 ]
  [ "$(jq '.pages.threadComments' "$f")" -ge 1 ]
  [ "$(jq '[.threads[] | has("commentsHasNextPage")] | any' "$f")" = false ]
}

# --- live -------------------------------------------------------------------

@test "collect-pr (live): PR#115 at --page-size 2 equals the default-page-size collection and direct gh counts (AC-2)" {
  live
  bash "$SCRIPTS/collect-pr.sh" --repo Falconiere/toolu --pr 115 --out "$TMP/p2.json" --page-size 2
  bash "$SCRIPTS/collect-pr.sh" --repo Falconiere/toolu --pr 115 --out "$TMP/p100.json"
  diff <(jq -S 'del(.collectedAt, .pageSize, .pages)' "$TMP/p2.json") <(jq -S 'del(.collectedAt, .pageSize, .pages)' "$TMP/p100.json")
  [ "$(jq '.pages.threads' "$TMP/p2.json")" -ge 9 ]
  [ "$(jq '.pages.threads' "$TMP/p100.json")" -eq 1 ]
  [ "$(jq '.threads | length' "$TMP/p2.json")" -eq "$(gh api graphql -F owner=Falconiere -F repo=toolu -F number=115 -f query='query($owner:String!,$repo:String!,$number:Int!){repository(owner:$owner,name:$repo){pullRequest(number:$number){reviewThreads(first:100){totalCount}}}}' --jq '.data.repository.pullRequest.reviewThreads.totalCount')" ]
  [ "$(jq '.reviews | length' "$TMP/p2.json")" -eq "$(gh api --paginate "repos/Falconiere/toolu/pulls/115/reviews?per_page=100" --jq 'length')" ]
  [ "$(jq '.comments | length' "$TMP/p2.json")" -eq "$(gh api --paginate "repos/Falconiere/toolu/issues/115/comments?per_page=100" --jq 'length')" ]
}

@test "collect-pr (live): PR#165 at --page-size 2 reports the 3-comment thread completely" {
  live
  bash "$SCRIPTS/collect-pr.sh" --repo Falconiere/toolu --pr 165 --out "$TMP/165.json" --page-size 2
  [ "$(jq '[.threads[].comments | length] | max' "$TMP/165.json")" -eq 3 ]
  [ "$(jq '.pages.threadComments' "$TMP/165.json")" -ge 1 ]
}

@test "collect-pr (live): a missing PR is a permanent api_error after one attempt" {
  live
  run bash "$SCRIPTS/collect-pr.sh" --repo Falconiere/toolu --pr 999999 --out "$TMP/s.json"
  [ "$status" -eq 3 ]
  doc=$(printf '%s\n' "$output" | grep '^{')
  [ "$(jq -r '.errors[0].attempts' <<<"$doc")" = 1 ]
  [ "$(jq -r '.errors[0].class' <<<"$doc")" = permanent ]
  [ ! -e "$TMP/s.json" ]
}

@test "collect-pr: the head-moved policy is the shared retry-once helper mapped to head_moved" {
  # The two-read head check and its retry live in one place each; pin both so
  # a refactor cannot silently drop the recollect or the exit code.
  grep -Fq 'pb_retry_on_rc "$PB_HEAD_MOVED_RC" 2 _collect_once' "$SCRIPTS/collect-pr.sh"
  grep -Fq 'pb_fail head_moved' "$SCRIPTS/collect-pr.sh"
  grep -Fq 'return "$PB_HEAD_MOVED_RC"' "$SCRIPTS/collect-pr.sh"
  grep -Fq 'head.moved' "$SCRIPTS/collect-pr.sh"
  [ "$(grep -c '_read_head)' "$SCRIPTS/collect-pr.sh")" -eq 2 ]
}
