# Gate coverage matrix

**Issue:** [#209](https://github.com/Falconiere/toolu/issues/209) (epic [#203](https://github.com/Falconiere/toolu/issues/203))  
**Final removal:** [#279](https://github.com/Falconiere/toolu/issues/279) (epic [#247](https://github.com/Falconiere/toolu/issues/247))
**Inventory:** `fixtures/gate-coverage/inventory.json`

**Check:** `cargo xtask gate-coverage`

Every live hook and built-in gate in this inventory is `port-native` and runs on the generated native launcher (#440 removed the Bun bundles). A `hooks.json` row is named after the hook it runs (`hook <name>`); a built-in gate's source is its module under `crates/core/engine/src/gates/`. The final-removal check (`cargo xtask final-removal`) also rejects tracked shell and Bats files, except the root curl installer `install.sh` (#457).

| id | source | plugin | event | classification | support | impl | host mechanism | bash | limits |
|----|--------|--------|-------|----------------|---------|------|----------------|------|--------|
| `ast-grep:hooks.json:SessionStart:hook_register:startup|resume|clear|compact` | `plugins/ast-grep/hooks/hooks.json` | ast-grep | SessionStart | port-native | required | #429/done | native | no | — |
| `documentation-gate:hooks.json:SessionStart:hook_check-toolu:startup|resume|clear|compact` | `plugins/documentation-gate/hooks/hooks.json` | documentation-gate | SessionStart | port-native | optional | done | native | no | — |
| `documentation-gate:hooks.json:SessionStart:hook_register:startup|resume|clear|compact` | `plugins/documentation-gate/hooks/hooks.json` | documentation-gate | SessionStart | port-native | optional | done | native | no | — |
| `epic-orchestrator:hooks.json:SessionStart:hook_check-binary:startup|resume|clear|compact` | `plugins/epic-orchestrator/hooks/hooks.json` | epic-orchestrator | SessionStart | port-native | required | #435/done | native | no | — |
| `epic-orchestrator:hooks.json:SessionStart:hook_check-deps:startup|resume|clear|compact` | `plugins/epic-orchestrator/hooks/hooks.json` | epic-orchestrator | SessionStart | port-native | required | #435/done | native | no | — |
| `epic-orchestrator:hooks.json:SessionStart:hook_engine-ensure:startup|resume|clear|compact` | `plugins/epic-orchestrator/hooks/hooks.json` | epic-orchestrator | SessionStart | port-native | required | #435/done | native | no | — |
| `jev:hooks.json:SessionStart:hook_check-binary:startup|resume|clear|compact` | `plugins/jev/hooks/hooks.json` | jev | SessionStart | port-native | required | #443/done | native | no | — |
| `jev:hooks.json:SessionStart:hook_session-start:startup|resume|clear|compact` | `plugins/jev/hooks/hooks.json` | jev | SessionStart | port-native | required | #269/done | native | no | — |
| `jev:hooks.json:UserPromptSubmit:hook_user-prompt-submit` | `plugins/jev/hooks/hooks.json` | jev | UserPromptSubmit | port-native | required | #271/done | native | no | — |
| `pr-babysit:hooks.json:SessionStart:hook_check-binary:startup|resume|clear|compact` | `plugins/pr-babysit/hooks/hooks.json` | pr-babysit | SessionStart | port-native | required | #443/done | native | no | — |
| `pr-babysit:hooks.json:SessionStart:hook_check-toolu:startup|resume|clear|compact` | `plugins/pr-babysit/hooks/hooks.json` | pr-babysit | SessionStart | port-native | required | #433/done | native | no | — |
| `python-quality:hooks.json:SessionStart:hook_check-toolu:startup|resume|clear|compact` | `plugins/python-quality/hooks/hooks.json` | python-quality | SessionStart | port-native | required | #427/done | native | no | — |
| `python-quality:hooks.json:SessionStart:hook_register:startup|resume|clear|compact` | `plugins/python-quality/hooks/hooks.json` | python-quality | SessionStart | port-native | required | #427/done | native | no | — |
| `rust-quality:hooks.json:SessionStart:hook_check-toolu:startup|resume|clear|compact` | `plugins/rust-quality/hooks/hooks.json` | rust-quality | SessionStart | port-native | required | #428/done | native | no | — |
| `rust-quality:hooks.json:SessionStart:hook_register:startup|resume|clear|compact` | `plugins/rust-quality/hooks/hooks.json` | rust-quality | SessionStart | port-native | required | #428/done | native | no | — |
| `statusline:hooks.json:SessionStart:hook_session-start:startup|resume|clear|compact` | `plugins/statusline/hooks/hooks.json` | statusline | SessionStart | port-native | required | #431/done | native | no | — |
| `toolu-review:hooks.json:SessionStart:hook_check-binary:startup|resume|clear|compact` | `plugins/toolu-review/hooks/hooks.json` | toolu-review | SessionStart | port-native | required | #443/done | native | no | — |
| `toolu-review:hooks.json:SessionStart:hook_session-start:startup|resume|clear|compact` | `plugins/toolu-review/hooks/hooks.json` | toolu-review | SessionStart | port-native | required | #269/done | native | no | — |
| `toolu:builtin-module:PostToolUse:gate-status` | `crates/core/engine/src/gates/gate_status.rs` | toolu | PostToolUse | port-native | required | #259/done | native | no | — |
| `toolu:builtin-module:PostToolUse:push-waiver` | `crates/core/engine/src/gates/push_waiver.rs` | toolu | PostToolUse | port-native | required | #259/done | native | no | — |
| `toolu:builtin-module:PreToolUse:bash-commands` | `crates/core/engine/src/gates/bash_commands.rs` | toolu | PreToolUse | port-native | required | #261/done | native | no | — |
| `toolu:builtin-module:PreToolUse:code-edit-rules` | `crates/core/engine/src/gates/code_edit_rules.rs` | toolu | PreToolUse | port-native | required | #419/done | native | no | — |
| `toolu:builtin-module:PreToolUse:commit-gate` | `crates/core/engine/src/gates/commit_gate.rs` | toolu | PreToolUse | port-native | required | #261/done | native | no | — |
| `toolu:builtin-module:PreToolUse:docs-sync` | `crates/core/engine/src/gates/docs_sync.rs` | toolu | PreToolUse | port-native | required | #262/done | native | no | — |
| `toolu:builtin-module:PreToolUse:mcp-blocker` | `crates/core/engine/src/gates/mcp_blocker.rs` | toolu | PreToolUse | port-native | required | #419/done | native | no | — |
| `toolu:builtin-module:PreToolUse:plan-ledger` | `crates/core/engine/src/gates/plan_ledger.rs` | toolu | PreToolUse | port-native | required | #262/done | native | no | — |
| `toolu:builtin-module:PreToolUse:protected-files` | `crates/core/engine/src/gates/protected_files.rs` | toolu | PreToolUse | port-native | required | #419/done | native | no | — |
| `toolu:builtin-module:PreToolUse:push-review` | `crates/core/engine/src/gates/push_review.rs` | toolu | PreToolUse | port-native | required | #262/done | native | no | — |
| `toolu:builtin-module:PreToolUse:quality-gate` | `crates/core/engine/src/gates/quality_gate.rs` | toolu | PreToolUse | port-native | required | #261/done | native | no | — |
| `toolu:entrypoint:PreToolUse:agent-tier` | `crates/toolu/src/agent_tier.rs` | toolu | PreToolUse | port-native | required | #262/done | native | no | — |
| `toolu:hooks.json:PostToolUse:hook_post-tools:apply_patch|Edit|Write|MultiEdit|Bash|Sh` | `plugins/toolu/hooks/hooks.json` | toolu | PostToolUse | port-native | required | #279/done | native | no | — |
| `toolu:hooks.json:PreCompact:hook_pre-compact:auto` | `plugins/toolu/hooks/hooks.json` | toolu | PreCompact | port-native | required | #263/done | native | no | — |
| `toolu:hooks.json:PreToolUse:hook_agent-tier:spawn_agent|Agent|Task` | `plugins/toolu/hooks/hooks.json` | toolu | PreToolUse | port-native | required | #262/done | native | no | — |
| `toolu:hooks.json:PreToolUse:hook_mcp-tools:mcp__` | `plugins/toolu/hooks/hooks.json` | toolu | PreToolUse | port-native | required | #260/done | native | no | — |
| `toolu:hooks.json:PreToolUse:hook_pre-tools:apply_patch|Edit|Write|MultiEdit|Bash|Sh` | `plugins/toolu/hooks/hooks.json` | toolu | PreToolUse | port-native | required | #258/done | native | no | — |
| `toolu:hooks.json:SessionStart:hook_check-binary:startup|resume|clear|compact` | `plugins/toolu/hooks/hooks.json` | toolu | SessionStart | port-native | required | #443/done | native | no | — |
| `toolu:hooks.json:SessionStart:hook_session-start:startup|resume|clear|compact` | `plugins/toolu/hooks/hooks.json` | toolu | SessionStart | port-native | required | #263/done | native | no | — |
| `toolu:hooks.json:UserPromptSubmit:hook_user-prompt-submit` | `plugins/toolu/hooks/hooks.json` | toolu | UserPromptSubmit | port-native | required | #263/done | native | no | — |
| `ts-quality:hooks.json:SessionStart:hook_check-toolu:startup|resume|clear|compact` | `plugins/ts-quality/hooks/hooks.json` | ts-quality | SessionStart | port-native | required | #426/done | native | no | — |
| `ts-quality:hooks.json:SessionStart:hook_register:startup|resume|clear|compact` | `plugins/ts-quality/hooks/hooks.json` | ts-quality | SessionStart | port-native | required | #426/done | native | no | — |

## Notes

- Native built-in gate rows point to their Rust source and the dispatcher hook that invokes them.
- This inventory records the implementation of each listed hook rather than claiming every host exposes every event.
