# GitHub writes

Run these only after the user confirmed the draft. Write each body to a file
in the skill's scratch directory `$DIR` first, then pass it with `-F body=@…`
so no body goes through shell quoting. Make one call at a time: GitHub
applies secondary rate limits to fast creation.

The link and dependency endpoints take an issue's numeric **id**, not its
number. Keep both from every create:

```bash
gh api "repos/$REPO/issues" -f title="Epic: <name>" -F body=@"$DIR/epic.md" -f 'labels[]=epic' --jq '"\(.number) \(.id)"'
gh api "repos/$REPO/issues" -f title="<subject>" -F body=@"$DIR/<slug>.md" --jq '"\(.number) \(.id)"'
```

Create the `epic` label first when `gh label list --repo "$REPO" --search epic`
does not list it: `gh label create epic --repo "$REPO" --color 3E4B9E`.

## Order

1. The `epic` label, when missing.
2. The epic issue.
3. Each child, blockers before the issues they block, so a child's
   `## Relationships` can name the real number of its blocker. Write that
   number into the body file before creating the child.
4. Each child's sub-issue link, in the draft's order:

   ```bash
   gh api "repos/$REPO/issues/$EPIC/sub_issues" -F sub_issue_id="$CHILD_ID"
   ```

5. Each dependency edge:

   ```bash
   gh api "repos/$REPO/issues/$CHILD/dependencies/blocked_by" -F issue_id="$BLOCKER_ID"
   ```

## When a call fails

Stop at the first failure. Do not repeat a create: a lost response may have
committed it, and a second POST makes a duplicate issue. Look before any retry,
with live REST listings: search results lag behind a fresh create. The first
lists your newest issues; the others page through every link and dependency.

```bash
LOGIN=$(gh api user --jq .login)
gh api "repos/$REPO/issues?state=all&creator=$LOGIN&sort=created&direction=desc&per_page=100" --jq '.[] | "\(.number) \(.title)"'
gh api --paginate "repos/$REPO/issues/$EPIC/sub_issues?per_page=100" --jq '.[].number'
gh api --paginate "repos/$REPO/issues/$CHILD/dependencies/blocked_by?per_page=100" --jq '.[].number'
```

Then report three lists: issues created (number and title), links missing, and
dependencies missing. Finish only the missing ones, and only after the user
agrees. Never close, delete or edit an issue this run did not create.
