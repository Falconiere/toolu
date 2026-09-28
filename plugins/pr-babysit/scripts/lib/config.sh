#!/usr/bin/env bash
# config.sh — the `prBabysit` block of toolu.config.json.
#
# Sourced after common.sh and hosts.sh. Reads the user file and the project
# file, deep-merges them (project wins), validates, and fills defaults. The
# file locations follow toolu's host adapter (plugins/toolu/hooks/lib/host.sh:
# toolu_config_root, toolu_project_config) — pr-babysit cannot source another
# plugin's code, so __tests__/config.bats pins the parity. The controller host
# is passed explicitly (claude | codex) instead of being sniffed.

# pb_config_keys -> the keys pb_config_load accepts, one per line. docs/config.md
# documents each (asserted by babysit-contract.bats).
pb_config_keys() { printf '%s\n' dispatch hosts prefer routing unattended jev; }

# pb_agent_home HOST -> the host's per-user install root (~/.claude or
# ~/.codex, or their overrides). The one place pr-babysit reads the user's
# home: toolu.config.json and jev.sh are installed there by definition.
pb_agent_home() {
  if [ "$1" = codex ]; then
    printf '%s\n' "${CODEX_HOME:-$HOME/.codex}"
  else
    printf '%s\n' "${CLAUDE_CONFIG_DIR:-$HOME/.claude}"
  fi
}

# pb_config_user_path HOST -> <agent config root>/toolu.config.json
pb_config_user_path() {
  printf '%s/toolu.config.json\n' "${TOOLU_CONFIG_DIR:-$(pb_agent_home "$1")}"
}

# pb_config_project_path HOST -> <project root>/<.claude|.codex>/toolu.config.json,
# or nothing outside a project. The project root is TOOLU_PROJECT_DIR or the
# working directory's git toplevel — the workflow runs from the repository
# root, so no host session variable is needed.
pb_config_project_path() {
  local root="${TOOLU_PROJECT_DIR:-}" dir
  [ -n "$root" ] || root=$(git rev-parse --show-toplevel 2>/dev/null || true)
  [ -n "$root" ] || return 0
  if [ -n "${TOOLU_PROJECT_CONFIG_DIRNAME:-}" ]; then
    dir="$TOOLU_PROJECT_CONFIG_DIRNAME"
  elif [ "$1" = codex ]; then
    dir=.codex
  else
    dir=.claude
  fi
  printf '%s/%s/toolu.config.json\n' "$root" "$dir"
}

# _pb_config_block FILE -> the file's prBabysit value ({} when absent; any
# other JSON value is returned as-is for pb_config_load to reject). A
# malformed file warns once on stderr and counts as {} — the toolu loader's rule.
_pb_config_block() {
  local file="$1"
  [ -n "$file" ] && [ -f "$file" ] || { echo '{}'; return 0; }
  if ! jq -e . "$file" >/dev/null 2>&1; then
    printf 'pr-babysit-config: malformed JSON in %s; ignoring\n' "$file" >&2
    echo '{}'
    return 0
  fi
  jq -c 'if type == "object" and has("prBabysit") then .prBabysit else {} end' "$file"
}

# _pb_config_object FILE -> that file's prBabysit object, or exit
# config_invalid when the value is not an object (a string, array or false).
_pb_config_object() {
  local out
  out=$(_pb_config_block "$1") || pb_fail config_invalid "could not read $1"
  jq -e 'type == "object"' >/dev/null <<<"$out" \
    || pb_fail config_invalid "prBabysit must be an object (in $1)" "$(jq -nc --arg f "$1" '{file: $f}')"
  printf '%s\n' "$out"
}

# pb_config_load HOST -> the validated, defaulted prBabysit object:
#   {dispatch, hosts[], prefer{tier:[host]}, routing{host:[4]}, unattended, jev}
# Invalid values exit config_invalid.
pb_config_load() {
  local host="$1" user="" project="" merged out
  pb_capture user _pb_config_object "$(pb_config_user_path "$host")"
  pb_capture project _pb_config_object "$(pb_config_project_path "$host")"
  merged=$(jq -cn --argjson u "$user" --argjson p "$project" '$u * $p') || pb_fail config_invalid "prBabysit could not be merged"
  out=$(jq -c --arg host "$host" --arg safe "$PB_SHELL_SAFE_RE" --argjson defaults "$(pb_default_routing_json)" '
    def kind: {"claude":"claude","claude-code":"claude","codex":"codex","cursor":"cursor","cursor-agent":"cursor"}
      [tostring | ascii_downcase | sub("^\\s+"; "") | sub("\\s+$"; "")];
    def tiers: ["trivial","standard","complex","critical"];
    def hostlist($where):
      if type != "array" then error("\($where) must be an array of hosts")
      else map(kind // error("\($where) names an unknown host; use claude, codex, cursor"))
        | reduce .[] as $h ([]; if any(.[]; . == $h) then . else . + [$h] end) end;
    def entry_ok: type == "object" and all(.model, .effort; . == null or (type == "string" and test($safe)));
    def flag($k): if has($k) then .[$k] else true end
      | if type == "boolean" then . else error("\($k) must be true or false") end;
    try (
      if type != "object" then error("prBabysit must be an object") else . end
      | (if has("dispatch") then .dispatch else "herdr" end) as $dispatch
      | if (["herdr","inline"] | index([$dispatch])) == null then error("dispatch must be herdr or inline") else . end
      | (if has("hosts") then (.hosts | hostlist("hosts")) else [$host] end) as $hosts
      | if ($hosts | length) == 0 then error("hosts must name at least one host") else . end
      | ((.prefer // {})
          | if type != "object" then error("prefer must be an object") else . end
          | with_entries(.key as $t
              | if (tiers | index([$t])) == null then error("prefer key \($t) is not a tier")
                else .value |= hostlist("prefer.\($t)") end)) as $prefer
      | ((.routing // {})
          | if type != "object" then error("routing must be an object") else . end
          | with_entries(.key as $raw
              | (.key | kind // error("routing names an unknown host \($raw)")) as $k
              | if (.value | type) != "array" or (.value | length) != 4 then error("routing.\($raw) must list 4 tiers")
                elif (.value | all(entry_ok) | not) then error("routing.\($raw) has a non-string or shell-unsafe model/effort")
                else {key: $k, value: .value} end)) as $overrides
      | {dispatch: $dispatch, hosts: $hosts, prefer: $prefer, routing: ($defaults + $overrides),
         unattended: flag("unattended"), jev: flag("jev")}
    ) catch {error: .}' <<<"$merged") || pb_fail config_invalid "prBabysit could not be read"
  if jq -e 'has("error")' <<<"$out" >/dev/null; then
    pb_fail config_invalid "prBabysit: $(jq -r '.error' <<<"$out")"
  fi
  printf '%s\n' "$out"
}
