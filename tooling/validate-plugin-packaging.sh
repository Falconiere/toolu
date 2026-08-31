#!/usr/bin/env bash
# Validate that every plugin is packaged consistently for Claude Code and Codex.
set -euo pipefail

ROOT="${PACKAGING_ROOT:-$(cd "${BASH_SOURCE%/*}/.." && pwd)}"
cd "$ROOT"

fail() {
  printf 'validate-plugin-packaging: %s\n' "$*" >&2
  exit 1
}

command -v jq >/dev/null 2>&1 || fail 'jq is required'
command -v bun >/dev/null 2>&1 || fail 'bun is required for TOML validation'

package_version="$(jq -er '.version | strings' package.json)" || fail 'package.json must contain a version'
claude_catalog='.claude-plugin/marketplace.json'
codex_catalog='.agents/plugins/marketplace.json'
release_config='release-please-config.json'
[ -f "$claude_catalog" ] || fail "missing $claude_catalog"
[ -f "$codex_catalog" ] || fail "missing $codex_catalog"
[ -f "$release_config" ] || fail "missing $release_config"
jq -e . "$claude_catalog" >/dev/null || fail "invalid JSON: $claude_catalog"
jq -e . "$codex_catalog" >/dev/null || fail "invalid JSON: $codex_catalog"
jq -e . "$release_config" >/dev/null || fail "invalid JSON: $release_config"

count=0
for claude_manifest in plugins/*/.claude-plugin/plugin.json; do
  [ -f "$claude_manifest" ] || continue
  plugin_root="${claude_manifest%/.claude-plugin/plugin.json}"
  name="${plugin_root#plugins/}"
  codex_manifest="$plugin_root/.codex-plugin/plugin.json"

  [ -f "$codex_manifest" ] || fail "$name is missing its Codex manifest"
  while IFS= read -r plugin_link; do
    link_target="$(readlink "$plugin_link")" || fail "$name has an unreadable symlink: $plugin_link"
    case "$link_target" in
      /*) resolved_target="$link_target" ;;
      *) resolved_target="$(cd "$(dirname "$plugin_link")" && realpath -m "$link_target")" ;;
    esac
    case "$resolved_target" in
      "$(cd "$plugin_root" && pwd -P)"/*) ;;
      *) fail "$name symlink escapes plugin root: $plugin_link -> $link_target" ;;
    esac
  done < <(find "$plugin_root" -type l -print)
  jq -e . "$claude_manifest" >/dev/null || fail "invalid JSON: $claude_manifest"
  jq -e . "$codex_manifest" >/dev/null || fail "invalid JSON: $codex_manifest"

  claude_identity="$(jq -ce '{name, version, description}' "$claude_manifest")"
  codex_identity="$(jq -ce '{name, version, description}' "$codex_manifest")"
  [ "$claude_identity" = "$codex_identity" ] || fail "$name has mismatched Claude/Codex identity metadata"
  [ "$(jq -er '.version' "$claude_manifest")" = "$package_version" ] || fail "$name Claude manifest version differs from package.json"
  [ "$(jq -er '.version' "$codex_manifest")" = "$package_version" ] || fail "$name Codex manifest version differs from package.json"
  for release_manifest in "$claude_manifest" "$codex_manifest"; do
    if ! jq -e --arg path "$release_manifest" \
      '.packages["."]["extra-files"][] | select(.type == "json" and .path == $path and .jsonpath == "$.version")' \
      "$release_config" >/dev/null; then
      fail "release-please is missing $release_manifest"
    fi
  done

  if [ -d "$plugin_root/skills" ]; then
    [ "$(jq -er '.skills' "$codex_manifest")" = './skills/' ] || fail "$name must declare ./skills/"
  else
    ! jq -e 'has("skills")' "$codex_manifest" >/dev/null || fail "$name declares skills without a skills directory"
  fi
  if [ -f "$plugin_root/hooks/hooks.json" ]; then
    [ "$(jq -er '.hooks' "$codex_manifest")" = './hooks/hooks.json' ] || fail "$name must declare ./hooks/hooks.json"
  else
    ! jq -e 'has("hooks")' "$codex_manifest" >/dev/null || fail "$name declares hooks without hooks/hooks.json"
  fi

  claude_source="$(jq -r --arg name "$name" '.plugins[] | select(.name == $name) | .source' "$claude_catalog")"
  [ "$claude_source" = "./plugins/$name" ] || fail "$name is missing or has the wrong Claude marketplace source"
  [ "$(jq --arg name "$name" '[.plugins[] | select(.name == $name)] | length' "$claude_catalog")" -eq 1 ] || fail "$name must appear exactly once in the Claude marketplace"
  claude_description="$(jq -r --arg name "$name" '.plugins[] | select(.name == $name) | .description' "$claude_catalog")"
  [ "$claude_description" = "$(jq -r '.description' "$claude_manifest")" ] || fail "$name marketplace description differs from its manifest"
  codex_entry="$(jq -ce --arg name "$name" '.plugins[] | select(.name == $name)' "$codex_catalog")"
  [ -n "$codex_entry" ] || fail "$name is missing from the Codex marketplace"
  [ "$(jq --arg name "$name" '[.plugins[] | select(.name == $name)] | length' "$codex_catalog")" -eq 1 ] || fail "$name must appear exactly once in the Codex marketplace"
  [ "$(jq -r '.source.source' <<<"$codex_entry")" = local ] || fail "$name Codex marketplace source must be local"
  [ "$(jq -r '.source.path' <<<"$codex_entry")" = "./plugins/$name" ] || fail "$name Codex marketplace source path is wrong"
  [ "$(jq -r '.policy.installation' <<<"$codex_entry")" = AVAILABLE ] || fail "$name Codex marketplace installation policy is wrong"
  [ "$(jq -r '.policy.authentication' <<<"$codex_entry")" = ON_INSTALL ] || fail "$name Codex marketplace authentication policy is wrong"
  [ "$(jq -r '.category' <<<"$codex_entry")" = Productivity ] || fail "$name Codex marketplace category is wrong"

  count=$((count + 1))
done

[ "$count" -eq 14 ] || fail "expected 14 plugins, found $count"
[ "$(jq '[.plugins[].name] | length' "$claude_catalog")" -eq "$count" ] || fail 'Claude marketplace count does not match plugin manifests'
[ "$(jq '[.plugins[].name] | length' "$codex_catalog")" -eq "$count" ] || fail 'Codex marketplace count does not match plugin manifests'

skill_count=0
for skill_file in plugins/*/skills/*/SKILL.md; do
  [ -f "$skill_file" ] || continue
  [ "$(sed -n '1p' "$skill_file")" = '---' ] || fail "$skill_file is missing opening frontmatter"
  skill_name="$(sed -n '2s/^name:[[:space:]]*//p' "$skill_file")"
  skill_description="$(sed -n '3s/^description:[[:space:]]*//p' "$skill_file")"
  [ -n "$skill_name" ] || fail "$skill_file is missing a frontmatter name"
  [ -n "$skill_description" ] || fail "$skill_file is missing a frontmatter description"
  [[ "$skill_name" =~ ^[a-z0-9-]+$ ]] || fail "$skill_file has an invalid skill name"
  [ "$skill_name" = "$(basename "$(dirname "$skill_file")")" ] || fail "$skill_file name differs from its directory"
  awk 'NR > 1 && $0 == "---" { found=1; exit } END { exit !found }' "$skill_file" || fail "$skill_file is missing closing frontmatter"
  skill_count=$((skill_count + 1))
done
[ "$skill_count" -eq 26 ] || fail "expected 26 discoverable skills, found $skill_count"

required_skills=(
  plugins/toolu/skills/commit/SKILL.md
  plugins/toolu/skills/review-and-commit/SKILL.md
  plugins/toolu/skills/setup/SKILL.md
  plugins/comemory/skills/setup/SKILL.md
  plugins/statusline/skills/status/SKILL.md
  plugins/pr-babysit/skills/babysit/SKILL.md
)
for required_skill in "${required_skills[@]}"; do
  [ -f "$required_skill" ] || fail "missing Codex command-equivalent skill: $required_skill"
done

agent_count=0
for agent_file in plugins/toolu/assets/agents/*.toml; do
  [ -f "$agent_file" ] || continue
  if ! AGENT_FILE="$agent_file" bun -e '
    const path = process.env.AGENT_FILE;
    try {
      const value = Bun.TOML.parse(await Bun.file(path).text());
      const text = (key) => typeof value[key] === "string" && value[key].trim().length > 0;
      if (!["name", "description", "model", "model_reasoning_effort", "sandbox_mode", "developer_instructions"].every(text)) throw new Error("missing field");
      if (!["low", "medium", "high", "xhigh", "max", "ultra"].includes(value.model_reasoning_effort)) throw new Error("invalid effort");
      if (!["read-only", "workspace-write"].includes(value.sandbox_mode)) throw new Error("invalid sandbox");
    } catch (_) {
      process.exit(1);
    }
  ' >/dev/null 2>&1; then
    fail "invalid agent TOML: $agent_file"
  fi
  [ "$(basename "$agent_file" .toml)" = "$(AGENT_FILE="$agent_file" bun -e 'const v=Bun.TOML.parse(await Bun.file(process.env.AGENT_FILE).text()); process.stdout.write(v.name)')" ] || fail "$agent_file name differs from its filename"
  agent_count=$((agent_count + 1))
done
[ "$agent_count" -eq 5 ] || fail "expected 5 Codex agent profiles, found $agent_count"

hook_count=0
for hook_file in plugins/*/hooks/hooks.json; do
  [ -f "$hook_file" ] || continue
  jq -e '
    (.hooks | type) == "object" and
    all(.hooks | to_entries[];
      (.value | type) == "array" and
      all(.value[];
        (.hooks | type) == "array" and
        all(.hooks[]; .type == "command" and (.command | type) == "string")))
  ' "$hook_file" >/dev/null || fail "invalid hook schema: $hook_file"
  plugin_root="${hook_file%/hooks/hooks.json}"
  while IFS= read -r hook_command; do
    case "$hook_command" in
      '${CLAUDE_PLUGIN_ROOT}/'*)
        hook_path="${hook_command#'${CLAUDE_PLUGIN_ROOT}/'}"
        ;;
      '${PLUGIN_ROOT}/'*)
        hook_path="${hook_command#'${PLUGIN_ROOT}/'}"
        ;;
      *)
        fail "$hook_file has a non-plugin-relative hook command: $hook_command"
        ;;
    esac
    [ -x "$plugin_root/$hook_path" ] || fail "$hook_file references a missing or non-executable hook: $hook_path"
  done < <(jq -r '.hooks | to_entries[].value[]?.hooks[]? | select(.type == "command") | .command' "$hook_file")
  hook_count=$((hook_count + 1))
done
[ "$hook_count" -eq 14 ] || fail "expected 14 hook manifests, found $hook_count"

printf 'validate-plugin-packaging: validated %d plugins, %d skills, %d agents, and %d hook manifests\n' \
  "$count" "$skill_count" "$agent_count" "$hook_count"
