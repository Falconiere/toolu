#!/usr/bin/env bats
# Every documented npx invocation of the CLI must be `npx @toolu/cli@latest`.
#
# npm exec checks the local project tree before the registry. For a bare name,
# or an exact version, any local package called @toolu/cli wins. Inside a toolu
# checkout that is the tools/toolu-cli workspace, which has no built bin, so the
# bare form printed `sh: toolu: command not found` from every worktree. In a
# user's project that already depends on the CLI, it silently ran that older
# copy. A dist-tag always resolves against the registry, so `@latest` fetches
# the newest release wherever it runs.
#
# The unscoped `npx toolu` names a package that does not exist on npm; npm
# rejected the name when the CLI was first published.

ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd)"

# Tracked markdown only: gitignored scratch specs are not published docs.
# CHANGELOG.md is release-please history and stays as it was written.
tracked_markdown() {
  git -C "$ROOT" ls-files -z -- '*.md' ':!CHANGELOG.md'
}

@test "documented npx invocations of the CLI all pin @latest" {
  local found offenders
  found=$(cd "$ROOT" && tracked_markdown |
    xargs -0 grep -noE 'npx( +-[[:alnum:]-]+)* +@toolu/cli[^[:space:]`]*' || true)
  # The pattern must still see the documented commands, or this test is vacuous.
  [ -n "$found" ]
  offenders=$(printf '%s\n' "$found" | grep -vE '@toolu/cli@latest$' || true)
  if [ -n "$offenders" ]; then
    printf 'use npx @toolu/cli@latest; these resolve a local package first:\n%s\n' "$offenders" >&2
    return 1
  fi
}

@test "no documented npx invocation uses the unscoped toolu package" {
  local offenders
  offenders=$(cd "$ROOT" && tracked_markdown |
    xargs -0 grep -nE 'npx( +-[[:alnum:]-]+)* +toolu([[:space:]`]|$)' || true)
  if [ -n "$offenders" ]; then
    printf 'the unscoped toolu package does not exist on npm; use npx @toolu/cli@latest:\n%s\n' "$offenders" >&2
    return 1
  fi
}
