#!/usr/bin/env bash
# VENDORED — DO NOT EDIT BY HAND. Byte-identical copy of stats_pricing_jq
# from plugins/stats/scripts/lib/pricing.sh. Cross-plugin `source` is
# impossible (separate CLAUDE_PLUGIN_ROOTs), so token-ledger.sh sources this
# sibling copy. Regenerate: bash plugins/statusline/scripts/sync-pricing.sh
# Drift is caught by plugins/statusline/__tests__/pricing-sync.bats.
_tl_pricing_jq() {
  cat <<'JQ'
def rates(m):
  if   (m | test("fable|mythos")) then {i: 10, o: 50}
  elif (m | test("opus"))         then {i: 5,  o: 25}
  elif (m | test("haiku"))        then {i: 1,  o: 5}
  elif (m | test("sonnet"))       then {i: 3,  o: 15}
  else                                 {i: 3,  o: 15, unknown: true} end;
def msgcost($u; $r):
  ($u.input_tokens // 0)                               as $inp
  | ($u.output_tokens // 0)                            as $outp
  | ($u.cache_read_input_tokens // 0)                  as $crd
  | ($u.cache_creation_input_tokens // 0)              as $cwr
  | ($u.cache_creation.ephemeral_5m_input_tokens // 0) as $w5
  | ($u.cache_creation.ephemeral_1h_input_tokens // 0) as $w1
  | ( $inp * $r.i + $outp * $r.o + $crd * $r.i * 0.1
    + (if ($w5 + $w1) > 0 then $w5 * 1.25 * $r.i + $w1 * 2 * $r.i
                          else $cwr * 1.25 * $r.i end)
    ) / 1000000;
JQ
}
