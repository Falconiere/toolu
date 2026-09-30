#!/usr/bin/env bats
# Parity gate for scripts/lib/hosts.sh against epic-orchestrator's hosts.ts and
# route.ts (AC-4). The oracle is epic's own TypeScript, evaluated live with bun
# — the repo's test runner requires bun, so a missing bun FAILS here rather
# than skipping. Literal anchors copied from epic's hosts.test.ts pin the argv
# even if both implementations drifted together. No mocks.

LIB="${BATS_TEST_DIRNAME}/../lib"
EPIC="$(cd "${BATS_TEST_DIRNAME}/../../../epic-orchestrator/scripts" && pwd -P)"
NAME=pb-3fa2c1-r1g1

setup() {
  TMP=$(mktemp -d)
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

with_hosts() {
  bash -c "set -euo pipefail; . '$LIB/fixer-compat.sh'; . '$LIB/hosts.sh'; $1"
}

# pb_args_json KIND MODEL EFFORT BYPASS -> the babysit argv as a JSON array
pb_args_json() {
  with_hosts "pb_agent_args '$1' '$NAME' '$2' '$3' '$4' auto" | jq -R . | jq -sc .
}

@test "bun is available: the parity oracle cannot be skipped" {
  command -v bun
}

@test "AC-4: 24-case argv matrix equals epic agentArgs (claude/codex/cursor x bypass/safe x model? x effort?)" {
  bun -e "
    import { agentArgs } from '$EPIC/hosts.ts';
    const models = { claude: 'opus', codex: 'gpt-6-sol', cursor: 'gpt-5.6-sol-high' };
    const out = [];
    for (const kind of ['claude', 'codex', 'cursor'])
      for (const bypass of [true, false])
        for (const model of [undefined, models[kind]])
          for (const effort of [undefined, 'high'])
            out.push({ kind, bypass, model: model ?? '', effort: effort ?? '',
              args: agentArgs(kind, { key: '$NAME', model, effort, bypass, permissionMode: 'auto', resume: false }) });
    console.log(JSON.stringify(out));
  " >"$TMP/oracle.json"
  [ "$(jq length "$TMP/oracle.json")" -eq 24 ]
  n=0
  # Tab is IFS whitespace, so empty fields would collapse: "-" stands for "unset".
  while IFS=$'\t' read -r kind bypass model effort want; do
    [ "$model" = - ] && model=""
    [ "$effort" = - ] && effort=""
    got=$(pb_args_json "$kind" "$model" "$effort" "$bypass")
    if [ "$got" != "$want" ]; then
      echo "mismatch: $kind bypass=$bypass model=$model effort=$effort" >&2
      echo "  epic:    $want" >&2
      echo "  babysit: $got" >&2
      return 1
    fi
    n=$((n + 1))
  done < <(jq -r '.[] | [.kind, (.bypass|tostring), (if .model == "" then "-" else .model end), (if .effort == "" then "-" else .effort end), (.args|tojson)] | @tsv' "$TMP/oracle.json")
  [ "$n" -eq 24 ]
}

@test "AC-4: literal anchors from epic hosts.test.ts" {
  [ "$(pb_args_json claude opus high true)" = "[\"--dangerously-skip-permissions\",\"-n\",\"$NAME\",\"--model\",\"opus\",\"--effort\",\"high\"]" ]
  [ "$(pb_args_json codex gpt-6-sol medium true)" = '["--dangerously-bypass-approvals-and-sandbox","--model","gpt-6-sol","-c","model_reasoning_effort=medium"]' ]
  [ "$(pb_args_json cursor gpt-5.6-sol-high high true)" = '["--yolo","--trust","--approve-mcps","--model","gpt-5.6-sol-high"]' ]
  [ "$(pb_args_json claude '' '' false | jq -c '.[0:2]')" = '["--permission-mode","auto"]' ]
  pb_args_json codex '' '' false | jq -e 'index("on-request") != null'
}

@test "AC-4: default routing table equals epic DEFAULT_TABLE for claude, codex and cursor" {
  want=$(bun -e "
    import { DEFAULT_TABLE } from '$EPIC/route.ts';
    const { claude, codex, cursor } = DEFAULT_TABLE.hosts;
    console.log(JSON.stringify({ claude, codex, cursor }));
  " | jq -S -c .)
  got=$(with_hosts 'pb_default_routing_json' | jq -S -c .)
  [ "$got" = "$want" ]
  [ "$(jq '.claude | length' <<<"$got")" -eq 4 ]
}

@test "AC-4: the usage-limit pattern is epic's HOST_LIMIT source, case-insensitive" {
  want=$(bun -e "import { HOST_LIMIT } from '$EPIC/hosts.ts'; console.log(JSON.stringify([HOST_LIMIT.source, HOST_LIMIT.flags]));")
  got=$(with_hosts 'jq -nc --arg re "$(pb_host_limit_re)" "[\$re, \"i\"]"')
  [ "$got" = "$want" ]
}

@test "AC-4: host aliases resolve like epic parseHostKind" {
  [ "$(with_hosts 'pb_host_kind cursor-agent')" = cursor ]
  [ "$(with_hosts 'pb_host_kind Claude-Code')" = claude ]
  [ "$(with_hosts 'pb_host_kind " Cursor-Agent "')" = cursor ]
  [ "$(with_hosts 'pb_host_kind codex')" = codex ]
  [ "$(with_hosts 'pb_host_cli cursor')" = cursor-agent ]
  [ "$(with_hosts 'pb_host_cli claude')" = claude ]
}

@test "AC-4 boundary: an unknown host is config_invalid (exit 3)" {
  run with_hosts 'pb_host_kind gemini'
  [ "$status" -eq 3 ]
  [ "$(jq -r '.errors[0].code' <<<"$output")" = config_invalid ]
}

@test "AC-4 boundary: args the pane shell would mangle are config_invalid (exit 3)" {
  run with_hosts "pb_agent_args claude $NAME 'a b' '' true auto"
  [ "$status" -eq 3 ]
  [ "$(jq -r '.errors[0].code' <<<"$output")" = config_invalid ]
  run with_hosts "pb_agent_args cursor $NAME 'claude[effort=high]' '' true auto"
  [ "$status" -eq 3 ]
  [ "$(jq -r '.errors[0].code' <<<"$output")" = config_invalid ]
}
