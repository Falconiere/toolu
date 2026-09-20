#!/usr/bin/env bash
# normalize.sh — the jq vocabulary shared by collect-pr.sh and reduce-state.sh.
#
# Sourced. Every shape rule lives here so the collector, the reducer and the
# tests read one definition of "CI reviewer", "check state" and "thread
# comment". Programs are functions that print jq text (not variables) so a
# sourced-only lib stays shellcheck-clean without exporting large strings into
# every child process's environment.

# CI-reviewer login set: REST (`user.login`) keeps the `[bot]` suffix, GraphQL
# (`author.login`) drops it. Exact membership — never a `[bot]` substring test,
# which misreads the GraphQL form as human.
pb_ci_reviewers_json() { printf '%s' '["github-actions","github-actions[bot]","claude","claude[bot]"]'; }

# pb_jq_defs -> jq `def`s to prepend to any program.
pb_jq_defs() {
  cat <<EOF
def ci_reviewers: $(pb_ci_reviewers_json);
def is_ci_reviewer: . as \$l | (ci_reviewers | index(\$l)) != null;
def check_state:
  if .__typename == "StatusContext" then
    (if .state == "SUCCESS" then "pass"
     elif (.state == "PENDING" or .state == "EXPECTED") then "pending"
     else "fail" end)
  else
    (if .status != "COMPLETED" then "pending"
     elif (.conclusion == "SUCCESS" or .conclusion == "NEUTRAL" or .conclusion == "SKIPPED") then "pass"
     else "fail" end)
  end;
def check_name: (.name // .context // "unknown");
def check_url: (.detailsUrl // .targetUrl // null);
def ci_status:
  (map(check_state)) as \$s
  | if (\$s | length) == 0 then "pending"
    elif (\$s | index("fail")) != null then "fail"
    elif (\$s | index("pending")) != null then "pending"
    else "pass" end;
def thread_comment: {id, databaseId, body, author: (.author.login // null), authorType: (.author.__typename // null), createdAt, url};
EOF
}

# Merge `gh api graphql --paginate --slurp` reviewThreads pages (array of page
# objects) into the snapshot thread shape.
pb_jq_threads_from_pages() {
  pb_jq_defs
  cat <<'EOF'
[.[].data.repository.pullRequest.reviewThreads.nodes[]]
| map({id, isResolved, isOutdated, path, line,
       comments: (.comments.nodes | map(thread_comment)),
       commentsHasNextPage: .comments.pageInfo.hasNextPage,
       commentsEndCursor: .comments.pageInfo.endCursor})
EOF
}

# Merge the per-thread `node(id:)` comment pages (array of page objects).
pb_jq_thread_comments_from_pages() {
  pb_jq_defs
  printf '%s\n' '[.[].data.node.comments.nodes[]] | map(thread_comment)'
}

# Flatten `gh api --paginate --slurp` REST pages (array of arrays).
pb_jq_rest_items() { printf '%s\n' '[.[][]]'; }

pb_jq_issue_comment() {
  printf '%s\n' '{id, body, author: (.user.login // null), authorType: (.user.type // null), createdAt: .created_at, updatedAt: .updated_at, url: .html_url}'
}

pb_jq_review() {
  printf '%s\n' '{id, state, body, author: (.user.login // null), authorType: (.user.type // null), submittedAt: .submitted_at, url: .html_url, commitId: .commit_id}'
}

# Fields requested from `gh pr view --json`.
pb_pr_view_fields() {
  printf '%s' 'number,title,url,author,state,baseRefName,headRefName,headRefOid,statusCheckRollup,mergeable,reviewDecision'
}

# GraphQL documents. Both declare $endCursor so `gh api graphql --paginate`
# drives the cursor; $pageSize keeps pagination testable on real PRs.
pb_gql_threads() {
  cat <<'EOF'
query($owner:String!,$repo:String!,$number:Int!,$pageSize:Int!,$endCursor:String){
  repository(owner:$owner,name:$repo){ pullRequest(number:$number){
    reviewThreads(first:$pageSize,after:$endCursor){
      pageInfo{hasNextPage endCursor}
      nodes{ id isResolved isOutdated path line
        comments(first:$pageSize){ pageInfo{hasNextPage endCursor}
          nodes{ id databaseId body author{login __typename} createdAt url } } } } } } }
EOF
}

pb_gql_thread_comments() {
  cat <<'EOF'
query($id:ID!,$pageSize:Int!,$endCursor:String){
  node(id:$id){ ... on PullRequestReviewThread {
    comments(first:$pageSize,after:$endCursor){ pageInfo{hasNextPage endCursor}
      nodes{ id databaseId body author{login __typename} createdAt url } } } } }
EOF
}
