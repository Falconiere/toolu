#!/usr/bin/env bash
# Gate enforcement modes — how a failed gate check reaches the user.
#
# Every toolu gate answers one question ("is this push reviewed?", "is the
# quality gate green?") and then has to decide how forcefully to say no. That
# second decision is this file's job, so a gate module owns its check and its
# reason text while the delivery is uniform and user-configurable.
#
# Modes:
#   block   permissionDecision "deny"   — the tool call does not happen
#   ask     permissionDecision "ask"    — the user is prompted, and decides
#   advise  additionalContext           — the agent is told, nothing is stopped
#   off     nothing at all
#
# Two CLASSES of gate, and the difference only shows up when `ask` cannot be
# delivered:
#
#   judgement gates (pushReview, qualityGate, commitGate, planLedger,
#     docsSync, agentTier) — soft calls about process. `ask` on a host that
#     cannot prompt degrades to `advise`: say the thing, don't stop the work.
#
#   SECURITY GUARDRAILS ($TOOLU_GATE_GUARDRAILS: protectedFiles, mcpBlocker,
#     bashCommands) — the ones standing between an agent and your secrets,
#     your git internals, the hook framework itself, and arbitrary code
#     execution. These now ASK by default instead of denying outright, so a
#     human decides in the moment. That only holds if the human can actually
#     be reached: on a host with no prompt, `ask` degrades to `block`, NOT to
#     `advise`. Degrading a guardrail to advise would silently turn it off
#     exactly where nobody is watching.
#
# Resolution precedence, first hit wins:
#   1. .gates.<name>.mode
#   2. the legacy top-level key (.docsSync.mode / .agentTier.mode only)
#   3. the .gates.preset table below
#   4. the built-in default preset (balanced)
#
# A value that is present but not a known mode warns once on stderr and falls
# through to the next layer — the same warn-and-fall-back discipline
# toolu_model uses, so a typo mis-delivers nothing.
#
# Public API:
#   toolu_gate_preset            print the resolved preset name
#   toolu_gate_mode NAME         print block|ask|advise|off for gate NAME
#   toolu_gate_emit MODE REASON  print the PreToolUse JSON for MODE
#   toolu_gate_guardrail_warning HEADLINE DETAIL
#                                compose a guardrail's `ask` prompt text

_TOOLU_GATE_MODE_LIB_DIR="$(cd "${BASH_SOURCE%/*}" && pwd)"
# shellcheck source=config.sh
. "$_TOOLU_GATE_MODE_LIB_DIR/config.sh"

TOOLU_GATE_MODES="block ask advise off"
TOOLU_GATE_PRESETS="strict balanced relaxed"
TOOLU_GATE_NAMES="pushReview qualityGate commitGate bashCommands planLedger docsSync agentTier protectedFiles mcpBlocker"
TOOLU_GATE_DEFAULT_PRESET="balanced"

# The security guardrails. Two things are special about this list:
#   1. `ask` degrades to `block` (not `advise`) where the host cannot prompt.
#   2. They ask at every preset except `strict`, which still denies outright.
# `off` remains available per-gate for anyone who wants it, deliberately and
# in writing, in their own config.
TOOLU_GATE_GUARDRAILS="protectedFiles mcpBlocker bashCommands"

# Gates whose pre-`gates.*` config key is still honored, as "<gate>:<path>".
# Only these two ever had a documented top-level mode key; a gate absent here
# has no legacy layer to consult.
_TOOLU_GATE_LEGACY_PATHS="docsSync:docsSync.mode agentTier:agentTier.mode"

_toolu_gate_warn() {
  printf 'toolu-gate: %s\n' "$1" >&2
}

# _toolu_gate_preset_mode PRESET NAME -> mode for NAME under PRESET.
# The table is the whole policy; balanced is what ships.
_toolu_gate_preset_mode() {
  local preset="$1" name="$2"
  case "$preset" in
    strict)
      printf 'block'
      ;;
    balanced)
      case "$name" in
        qualityGate)                            printf 'block' ;;
        protectedFiles|mcpBlocker|bashCommands) printf 'ask' ;;
        *)                                      printf 'advise' ;;
      esac
      ;;
    relaxed)
      case "$name" in
        # `relaxed` relaxes judgement calls. A guardrail still asks: relaxed
        # means "stop lecturing me", not "write my .env without telling me".
        protectedFiles|mcpBlocker|bashCommands) printf 'ask' ;;
        pushReview|qualityGate)                 printf 'advise' ;;
        *)                                      printf 'off' ;;
      esac
      ;;
    *)
      return 1
      ;;
  esac
}

# toolu_gate_preset -> the configured preset, or the built-in default.
toolu_gate_preset() {
  # shellcheck disable=SC2086  # intentional word-split of the allowed-value list
  toolu_string gates.preset "$TOOLU_GATE_DEFAULT_PRESET" $TOOLU_GATE_PRESETS
}

# _toolu_gate_read_mode PATH -> print the mode at PATH, or return 1.
# Returns 1 both when the key is absent and when it held something that is not
# a mode (toolu_string has already warned in the latter case), which is exactly
# the "fall through to the next layer" behavior both cases want.
_toolu_gate_read_mode() {
  local value
  # shellcheck disable=SC2086  # intentional word-split of the allowed-value list
  value=$(toolu_string "$1" "__unset__" $TOOLU_GATE_MODES)
  [ "$value" = "__unset__" ] && return 1
  printf '%s' "$value"
}

# _toolu_gate_legacy_path NAME -> print NAME's legacy config path, or return 1.
_toolu_gate_legacy_path() {
  local name="$1" entry
  for entry in $_TOOLU_GATE_LEGACY_PATHS; do
    if [ "${entry%%:*}" = "$name" ]; then
      printf '%s' "${entry#*:}"
      return 0
    fi
  done
  return 1
}

# toolu_gate_mode NAME -> block|ask|advise|off.
#
# An unknown gate name is a caller typo, not a config problem: warn and print
# `block`, so a misspelled gate keeps enforcing rather than silently opening.
toolu_gate_mode() {
  local name="$1" mode="" legacy_path preset
  case " $TOOLU_GATE_NAMES " in
    *" $name "*) ;;
    *)
      _toolu_gate_warn "unknown gate '$name' (known: $TOOLU_GATE_NAMES); enforcing block"
      printf 'block'
      return 1
      ;;
  esac

  mode=$(_toolu_gate_read_mode "gates.${name}.mode") || mode=""

  if [ -z "$mode" ] && legacy_path=$(_toolu_gate_legacy_path "$name"); then
    mode=$(_toolu_gate_read_mode "$legacy_path") || mode=""
  fi

  if [ -z "$mode" ]; then
    preset=$(toolu_gate_preset)
    mode=$(_toolu_gate_preset_mode "$preset" "$name") || mode=""
  fi

  # Both fallbacks exhausted (only reachable if the preset table and the name
  # list ever drift apart): enforce rather than open.
  [ -n "$mode" ] || mode="block"

  # `ask` needs a host that can prompt. Where none can, the gate still speaks —
  # but WHAT it degrades to depends on the gate's class:
  #
  #   judgement gate  -> advise. Say it, don't stop the work.
  #   guardrail       -> block.  The whole point of asking is that a human
  #                      decides; with no human reachable the safe answer is
  #                      no. Degrading these to advise would hand an agent
  #                      silent write access to .env on exactly the hosts
  #                      where nobody is watching.
  if [ "$mode" = "ask" ] && ! toolu_supports_ask; then
    case " $TOOLU_GATE_GUARDRAILS " in
      *" $name "*) mode="block" ;;
      *)           mode="advise" ;;
    esac
  fi

  printf '%s' "$mode"
}

# toolu_gate_guardrail_warning HEADLINE DETAIL
# Compose the prompt text for a SECURITY GUARDRAIL's `ask`.
#
# Loud on purpose. Answering yes here overrides a protection that exists
# precisely so an agent cannot do this quietly, and a prompt that reads like an
# ordinary "allow this command?" trains people to wave it through. The banner,
# the WHY, and the "this call only" line are the three things a person needs to
# make a real decision in the two seconds they will actually spend on it.
toolu_gate_guardrail_warning() {
  local headline="$1" detail="$2"
  printf '%s' "\
############################################################
##  ⚠️  SECURITY GUARDRAIL — OVERRIDE REQUESTED  ⚠️        ##
############################################################

$headline

WHY THIS IS GUARDED
$detail

Approving covers THIS ONE CALL. Nothing is remembered and the next attempt
asks again. If you did not just ask for this, the answer is no."
}

# toolu_gate_emit MODE REASON
# Print the PreToolUse hook JSON for MODE carrying REASON. `off` prints
# nothing. An unknown MODE is treated as `block`: a delivery bug must not
# become a silent allow.
toolu_gate_emit() {
  local mode="$1" reason="$2"
  case "$mode" in
    off)
      return 0
      ;;
    advise)
      jq -n --arg reason "$reason" '{
        hookSpecificOutput: {
          hookEventName: "PreToolUse",
          additionalContext: $reason
        }
      }'
      ;;
    ask)
      jq -n --arg reason "$reason" '{
        hookSpecificOutput: {
          hookEventName: "PreToolUse",
          permissionDecision: "ask",
          permissionDecisionReason: $reason
        }
      }'
      ;;
    *)
      [ "$mode" = "block" ] || _toolu_gate_warn "unknown mode '$mode'; emitting block"
      jq -n --arg reason "$reason" '{
        hookSpecificOutput: {
          hookEventName: "PreToolUse",
          permissionDecision: "deny",
          permissionDecisionReason: $reason
        }
      }'
      ;;
  esac
}
