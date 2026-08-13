#!/usr/bin/env bats
# Cross-plugin guard for the host-native roots used by SessionStart publishers.

ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd)"

@test "Codex SessionStart wrapper publishers use CODEX_HOME" {
  local plugin rel hook codex_home isolated_home
  isolated_home="$BATS_TEST_TMPDIR/home"
  for item in \
    'agent-browser:agent-browser/agent-browser.sh' \
    'context7:context7/search.sh' \
    'exa-search:exa-search/search.sh' \
    'jira:jira/jira.sh' \
    'statusline:statusline/statusline.sh' \
    'toolu-review:toolu-review/write-state.sh'; do
    plugin=${item%%:*}
    rel=${item#*:}
    hook="$ROOT/plugins/$plugin/hooks/session-start.sh"
    codex_home="$BATS_TEST_TMPDIR/codex-$plugin"
    run env -u CLAUDE_CONFIG_DIR HOME="$isolated_home" CODEX_HOME="$codex_home" \
      PLUGIN_ROOT="$ROOT/plugins/$plugin" bash "$hook" <<<'{}'
    [ "$status" -eq 0 ]
    [ -L "$codex_home/$rel" ]
    [ ! -e "$isolated_home/.claude/$rel" ]
  done
}

@test "Codex registry publishers use the shared CODEX_HOME registry" {
  local plugin hook codex_home isolated_home expected
  isolated_home="$BATS_TEST_TMPDIR/home"
  for item in \
    'ast-grep:pre-tools.d/ast-grep@toolu__search-nudge.sh' \
    'comemory:pre-tools.d/comemory@toolu__comemory-scope.sh' \
    'git-better:pre-tools.d/git-better@toolu__git-lean-nudge.sh' \
    'rust-quality:post-tools.d/rust-quality@toolu__rust-quality.sh' \
    'ts-quality:post-tools.d/ts-quality@toolu__ts-quality.sh'; do
    plugin=${item%%:*}
    expected=${item#*:}
    hook="$ROOT/plugins/$plugin/hooks/register.sh"
    codex_home="$BATS_TEST_TMPDIR/codex-$plugin"
    run env -u CLAUDE_CONFIG_DIR HOME="$isolated_home" CODEX_HOME="$codex_home" \
      PLUGIN_ROOT="$ROOT/plugins/$plugin" bash "$hook" <<<'{}'
    [ "$status" -eq 0 ]
    [ -f "$codex_home/toolu/$expected" ]
    [ ! -e "$isolated_home/.claude/toolu/$expected" ]
  done
}
