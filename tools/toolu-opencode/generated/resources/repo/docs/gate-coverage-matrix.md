# Gate coverage matrix

**Issue:** [#209](https://github.com/Falconiere/toolu/issues/209) (epic [#203](https://github.com/Falconiere/toolu/issues/203))\
**Final removal:** [#279](https://github.com/Falconiere/toolu/issues/279) (epic [#247](https://github.com/Falconiere/toolu/issues/247))
**Inventory:** `fixtures/gate-coverage/inventory.json`

**Check:** `bun run tooling/src/gate-coverage-inventory.ts check`

Every live hook and built-in gate in this inventory is `port-native`. The host mechanism records whether its entry uses a Bun bundle or the generated native launcher. The final-removal check also rejects tracked shell and Bats files, except the root curl installer `install.sh` (#457).

| id | source | plugin | event | classification | support | impl | host mechanism | bash | limits |
|----|--------|--------|-------|----------------|---------|------|----------------|------|--------|
| `ast-grep:hooks.json:SessionStart:check-binary.js:startup|resume|clear|compact` | `plugins/ast-grep/hooks/hooks.json` | ast-grep | SessionStart | port-native | required | #443/done | bun-bundle | no | — |
| `ast-grep:hooks.json:SessionStart:register.js:startup|resume|clear|compact` | `plugins/ast-grep/hooks/hooks.json` | ast-grep | SessionStart | port-native | required | #268/done | bun-bundle | no | — |
| `epic-orchestrator:hooks.json:SessionStart:check-binary.js:startup|resume|clear|compact` | `plugins/epic-orchestrator/hooks/hooks.json` | epic-orchestrator | SessionStart | port-native | required | #443/done | bun-bundle | no | — |
| `epic-orchestrator:hooks.json:SessionStart:check-deps.js:startup|resume|clear|compact` | `plugins/epic-orchestrator/hooks/hooks.json` | epic-orchestrator | SessionStart | port-native | required | #269/done | bun-bundle | no | — |
| `jev:hooks.json:SessionStart:check-binary.js:startup|resume|clear|compact` | `plugins/jev/hooks/hooks.json` | jev | SessionStart | port-native | required | #443/done | bun-bundle | no | — |
| `jev:hooks.json:SessionStart:session-start.js:startup|resume|clear|compact` | `plugins/jev/hooks/hooks.json` | jev | SessionStart | port-native | required | #269/done | bun-bundle | no | — |
| `jev:hooks.json:UserPromptSubmit:user-prompt-submit.js` | `plugins/jev/hooks/hooks.json` | jev | UserPromptSubmit | port-native | required | #271/done | bun-bundle | no | — |
| `pr-babysit:hooks.json:SessionStart:check-binary.js:startup|resume|clear|compact` | `plugins/pr-babysit/hooks/hooks.json` | pr-babysit | SessionStart | port-native | required | #443/done | bun-bundle | no | — |
| `pr-babysit:hooks.json:SessionStart:check-toolu.js:startup|resume|clear|compact` | `plugins/pr-babysit/hooks/hooks.json` | pr-babysit | SessionStart | port-native | required | #269/done | bun-bundle | no | — |
| `python-quality:hooks.json:SessionStart:check-toolu.js:startup|resume|clear|compact` | `plugins/python-quality/hooks/hooks.json` | python-quality | SessionStart | port-native | required | #269/done | bun-bundle | no | — |
| `python-quality:hooks.json:SessionStart:register.js:startup|resume|clear|compact` | `plugins/python-quality/hooks/hooks.json` | python-quality | SessionStart | port-native | required | #266/done | bun-bundle | no | — |
| `rust-quality:hooks.json:SessionStart:check-toolu.js:startup|resume|clear|compact` | `plugins/rust-quality/hooks/hooks.json` | rust-quality | SessionStart | port-native | required | #269/done | bun-bundle | no | — |
| `rust-quality:hooks.json:SessionStart:register.js:startup|resume|clear|compact` | `plugins/rust-quality/hooks/hooks.json` | rust-quality | SessionStart | port-native | required | #267/done | bun-bundle | no | — |
| `statusline:hooks.json:SessionStart:check-binary.js:startup|resume|clear|compact` | `plugins/statusline/hooks/hooks.json` | statusline | SessionStart | port-native | required | #443/done | bun-bundle | no | — |
| `statusline:hooks.json:SessionStart:session-start.js:startup|resume|clear|compact` | `plugins/statusline/hooks/hooks.json` | statusline | SessionStart | port-native | required | #269/done | bun-bundle | no | — |
| `toolu-review:hooks.json:SessionStart:check-binary.js:startup|resume|clear|compact` | `plugins/toolu-review/hooks/hooks.json` | toolu-review | SessionStart | port-native | required | #443/done | bun-bundle | no | — |
| `toolu-review:hooks.json:SessionStart:session-start.js:startup|resume|clear|compact` | `plugins/toolu-review/hooks/hooks.json` | toolu-review | SessionStart | port-native | required | #269/done | bun-bundle | no | — |
| `toolu:builtin-module:PostToolUse:gate-status` | `packages/toolu-core/src/gates/gate-status.ts` | toolu | PostToolUse | port-native | required | #259/done | bun-bundle | no | — |
| `toolu:builtin-module:PostToolUse:push-waiver` | `packages/toolu-core/src/gates/push-waiver.ts` | toolu | PostToolUse | port-native | required | #259/done | bun-bundle | no | — |
| `toolu:builtin-module:PreToolUse:bash-commands` | `packages/toolu-core/src/gates/bash-commands.ts` | toolu | PreToolUse | port-native | required | #261/done | bun-bundle | no | — |
| `toolu:builtin-module:PreToolUse:code-edit-rules` | `packages/toolu-core/src/gates/code-edit-rules.ts` | toolu | PreToolUse | port-native | required | #260/done | bun-bundle | no | — |
| `toolu:builtin-module:PreToolUse:commit-gate` | `packages/toolu-core/src/gates/commit-gate.ts` | toolu | PreToolUse | port-native | required | #261/done | bun-bundle | no | — |
| `toolu:builtin-module:PreToolUse:docs-sync` | `packages/toolu-core/src/gates/docs-sync.ts` | toolu | PreToolUse | port-native | required | #262/done | bun-bundle | no | — |
| `toolu:builtin-module:PreToolUse:mcp-blocker` | `packages/toolu-core/src/gates/mcp-blocker.ts` | toolu | PreToolUse | port-native | required | #260/done | bun-bundle | no | — |
| `toolu:builtin-module:PreToolUse:plan-ledger` | `packages/toolu-core/src/gates/plan-ledger.ts` | toolu | PreToolUse | port-native | required | #262/done | bun-bundle | no | — |
| `toolu:builtin-module:PreToolUse:protected-files` | `packages/toolu-core/src/gates/protected-files.ts` | toolu | PreToolUse | port-native | required | #260/done | bun-bundle | no | — |
| `toolu:builtin-module:PreToolUse:push-review` | `packages/toolu-core/src/gates/push-review.ts` | toolu | PreToolUse | port-native | required | #262/done | bun-bundle | no | — |
| `toolu:builtin-module:PreToolUse:quality-gate` | `packages/toolu-core/src/gates/quality-gate.ts` | toolu | PreToolUse | port-native | required | #261/done | bun-bundle | no | — |
| `toolu:entrypoint:PreToolUse:agent-tier` | `plugins/toolu/hooks/src/agent-tier.ts` | toolu | PreToolUse | port-native | required | #262/done | bun-bundle | no | — |
| `toolu:hooks.json:PostToolUse:post-tools.js:apply_patch|Edit|Write|MultiEdit|Bash|Sh` | `plugins/toolu/hooks/hooks.json` | toolu | PostToolUse | port-native | required | #279/done | bun-bundle | no | — |
| `toolu:hooks.json:PreCompact:pre-compact.js:auto` | `plugins/toolu/hooks/hooks.json` | toolu | PreCompact | port-native | required | #263/done | bun-bundle | no | — |
| `toolu:hooks.json:PreToolUse:agent-tier.js:spawn_agent|Agent|Task` | `plugins/toolu/hooks/hooks.json` | toolu | PreToolUse | port-native | required | #262/done | bun-bundle | no | — |
| `toolu:hooks.json:PreToolUse:mcp-tools.js:mcp__` | `plugins/toolu/hooks/hooks.json` | toolu | PreToolUse | port-native | required | #260/done | bun-bundle | no | — |
| `toolu:hooks.json:PreToolUse:pre-tools.js:apply_patch|Edit|Write|MultiEdit|Bash|Sh` | `plugins/toolu/hooks/hooks.json` | toolu | PreToolUse | port-native | required | #258/done | bun-bundle | no | — |
| `toolu:hooks.json:SessionStart:check-binary.js:startup|resume|clear|compact` | `plugins/toolu/hooks/hooks.json` | toolu | SessionStart | port-native | required | #443/done | bun-bundle | no | — |
| `toolu:hooks.json:SessionStart:session-start.js:startup|resume|clear|compact` | `plugins/toolu/hooks/hooks.json` | toolu | SessionStart | port-native | required | #263/done | bun-bundle | no | — |
| `toolu:hooks.json:UserPromptSubmit:user-prompt-submit.js` | `plugins/toolu/hooks/hooks.json` | toolu | UserPromptSubmit | port-native | required | #263/done | bun-bundle | no | — |
| `ts-quality:hooks.json:SessionStart:check-toolu.js:startup|resume|clear|compact` | `plugins/ts-quality/hooks/hooks.json` | ts-quality | SessionStart | port-native | required | #265/done | bun-bundle | no | — |
| `ts-quality:hooks.json:SessionStart:register.js:startup|resume|clear|compact` | `plugins/ts-quality/hooks/hooks.json` | ts-quality | SessionStart | port-native | required | #265/done | bun-bundle | no | — |

## Notes

- Native built-in gate rows point to their TypeScript source and the dispatcher hook that invokes them.
- OpenCode supports the host events documented in [conformance-report.md](conformance-report.md); this inventory records the implementation of each listed hook rather than claiming every host exposes every event.
