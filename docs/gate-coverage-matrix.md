# Gate coverage matrix

**Issue:** [#209](https://github.com/Falconiere/toolu/issues/209) (epic [#203](https://github.com/Falconiere/toolu/issues/203))  
**Inventory:** `tooling/fixtures/gate-coverage/inventory.json`  
**Check:** `bun run tooling/src/gate-coverage-inventory.ts check`

Classifications match [docs/portable-core.md](portable-core.md): `shell-out` · `port-native` · `port-new` · `no-map`.

| id | source | plugin | event | classification | support | impl | host mechanism | bash | limits |
|----|--------|--------|-------|----------------|---------|------|----------------|------|--------|
| `agent-browser:hooks.json:SessionStart:session-start.js:startup|resume|clear|compact` | `plugins/agent-browser/hooks/hooks.json` | agent-browser | SessionStart | shell-out | required | #210/todo | pending-opencode | no | — |
| `ast-grep:hooks.json:SessionStart:register.js:startup|resume|clear|compact` | `plugins/ast-grep/hooks/hooks.json` | ast-grep | SessionStart | shell-out | required | #210/todo | pending-opencode | no | — |
| `context7:hooks.json:SessionStart:session-start.js:startup|resume|clear|compact` | `plugins/context7/hooks/hooks.json` | context7 | SessionStart | shell-out | required | #210/todo | pending-opencode | no | — |
| `epic-orchestrator:hooks.json:SessionStart:check-deps.js:startup|resume|clear|compact` | `plugins/epic-orchestrator/hooks/hooks.json` | epic-orchestrator | SessionStart | shell-out | required | #210/todo | pending-opencode | no | — |
| `exa-search:hooks.json:SessionStart:session-start.js:startup|resume|clear|compact` | `plugins/exa-search/hooks/hooks.json` | exa-search | SessionStart | shell-out | required | #210/todo | pending-opencode | no | — |
| `jev:hooks.json:SessionStart:session-start.js:startup|resume|clear|compact` | `plugins/jev/hooks/hooks.json` | jev | SessionStart | shell-out | required | #210/todo | pending-opencode | no | — |
| `jev:hooks.json:UserPromptSubmit:user-prompt-submit.js` | `plugins/jev/hooks/hooks.json` | jev | UserPromptSubmit | shell-out | required | #210/todo | pending-opencode | no | — |
| `jira:hooks.json:SessionStart:session-start.js:startup|resume|clear|compact` | `plugins/jira/hooks/hooks.json` | jira | SessionStart | shell-out | required | #210/todo | pending-opencode | no | — |
| `pr-babysit:hooks.json:SessionStart:check-toolu.js:startup|resume|clear|compact` | `plugins/pr-babysit/hooks/hooks.json` | pr-babysit | SessionStart | shell-out | required | #210/todo | pending-opencode | no | — |
| `python-quality:hooks.json:SessionStart:check-toolu.js:startup|resume|clear|compact` | `plugins/python-quality/hooks/hooks.json` | python-quality | SessionStart | shell-out | required | #204/todo | pending-opencode | no | — |
| `python-quality:hooks.json:SessionStart:register.js:startup|resume|clear|compact` | `plugins/python-quality/hooks/hooks.json` | python-quality | SessionStart | shell-out | required | #204/todo | pending-opencode | no | — |
| `rust-quality:hooks.json:SessionStart:check-toolu.js:startup|resume|clear|compact` | `plugins/rust-quality/hooks/hooks.json` | rust-quality | SessionStart | shell-out | required | #204/todo | pending-opencode | no | — |
| `rust-quality:hooks.json:SessionStart:register.js:startup|resume|clear|compact` | `plugins/rust-quality/hooks/hooks.json` | rust-quality | SessionStart | shell-out | required | #204/todo | pending-opencode | no | — |
| `statusline:hooks.json:SessionStart:session-start.js:startup|resume|clear|compact` | `plugins/statusline/hooks/hooks.json` | statusline | SessionStart | shell-out | required | #210/todo | pending-opencode | no | — |
| `toolu-review:hooks.json:SessionStart:session-start.js:startup|resume|clear|compact` | `plugins/toolu-review/hooks/hooks.json` | toolu-review | SessionStart | shell-out | required | #210/todo | pending-opencode | no | — |
| `toolu:builtin-module:PostToolUse:gate-status.sh` | `plugins/toolu/hooks/post-tools/modules/gate-status.sh` | toolu | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `toolu:builtin-module:PostToolUse:push-waiver.sh` | `plugins/toolu/hooks/post-tools/modules/push-waiver.sh` | toolu | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `toolu:builtin-module:PreToolUse:bash-commands` | `packages/toolu-core/src/gates/bash-commands.ts` | toolu | PreToolUse | port-native | required | #261/done | pending-opencode | no | — |
| `toolu:builtin-module:PreToolUse:code-edit-rules` | `packages/toolu-core/src/gates/code-edit-rules.ts` | toolu | PreToolUse | port-native | required | #260/done | bun-bundle | no | — |
| `toolu:builtin-module:PreToolUse:commit-gate` | `packages/toolu-core/src/gates/commit-gate.ts` | toolu | PreToolUse | port-native | required | #261/done | pending-opencode | no | — |
| `toolu:builtin-module:PreToolUse:docs-sync` | `packages/toolu-core/src/gates/docs-sync.ts` | toolu | PreToolUse | port-native | required | #262/done | bun-bundle | no | — |
| `toolu:builtin-module:PreToolUse:mcp-blocker` | `packages/toolu-core/src/gates/mcp-blocker.ts` | toolu | PreToolUse | port-native | required | #260/done | bun-bundle | no | — |
| `toolu:builtin-module:PreToolUse:plan-ledger` | `packages/toolu-core/src/gates/plan-ledger.ts` | toolu | PreToolUse | port-native | required | #262/done | bun-bundle | no | — |
| `toolu:builtin-module:PreToolUse:protected-files` | `packages/toolu-core/src/gates/protected-files.ts` | toolu | PreToolUse | port-native | required | #260/done | bun-bundle | no | — |
| `toolu:builtin-module:PreToolUse:push-review` | `packages/toolu-core/src/gates/push-review.ts` | toolu | PreToolUse | port-native | required | #262/done | bun-bundle | no | — |
| `toolu:builtin-module:PreToolUse:quality-gate` | `packages/toolu-core/src/gates/quality-gate.ts` | toolu | PreToolUse | port-native | required | #261/done | pending-opencode | no | — |
| `toolu:entrypoint:PreToolUse:agent-tier` | `plugins/toolu/hooks/src/agent-tier.ts` | toolu | PreToolUse | port-native | required | #262/done | bun-bundle | no | — |
| `toolu:hooks.json:PostToolUse:post-tools.js:apply_patch|Edit|Write|MultiEdit|Bash|Sh` | `plugins/toolu/hooks/hooks.json` | toolu | PostToolUse | shell-out | required | #210/todo | pending-opencode | yes | — |
| `toolu:hooks.json:PreCompact:pre-compact.js:auto` | `plugins/toolu/hooks/hooks.json` | toolu | PreCompact | shell-out | required | #210/todo | pending-opencode | no | — |
| `toolu:hooks.json:PreToolUse:agent-tier.js:spawn_agent|Agent|Task` | `plugins/toolu/hooks/hooks.json` | toolu | PreToolUse | port-native | required | #262/done | bun-bundle | no | — |
| `toolu:hooks.json:PreToolUse:mcp-tools.js:mcp__` | `plugins/toolu/hooks/hooks.json` | toolu | PreToolUse | port-native | required | #260/done | bun-bundle | no | — |
| `toolu:hooks.json:PreToolUse:pre-tools.js:apply_patch|Edit|Write|MultiEdit|Bash|Sh` | `plugins/toolu/hooks/hooks.json` | toolu | PreToolUse | port-native | required | #258/done | bun-bundle | no | — |
| `toolu:hooks.json:SessionStart:session-start.js:startup|resume|clear|compact` | `plugins/toolu/hooks/hooks.json` | toolu | SessionStart | shell-out | required | #210/todo | pending-opencode | no | — |
| `toolu:hooks.json:UserPromptSubmit:user-prompt-submit.js` | `plugins/toolu/hooks/hooks.json` | toolu | UserPromptSubmit | shell-out | required | #210/todo | pending-opencode | no | — |
| `toolu:lib:dependency:config.sh` | `plugins/toolu/hooks/lib/config.sh` | toolu | dependency | shell-out | supported | #210/todo | native-bash | yes | — |
| `toolu:lib:dependency:detect.sh` | `plugins/toolu/hooks/lib/detect.sh` | toolu | dependency | shell-out | supported | #210/todo | native-bash | yes | — |
| `toolu:lib:dependency:diff-sha.sh` | `plugins/toolu/hooks/lib/diff-sha.sh` | toolu | dependency | shell-out | supported | #210/todo | native-bash | yes | — |
| `toolu:lib:dependency:dispatch.sh` | `plugins/toolu/hooks/lib/dispatch.sh` | toolu | dependency | shell-out | supported | #210/todo | native-bash | yes | — |
| `toolu:lib:dependency:docs-sync-config.sh` | `plugins/toolu/hooks/lib/docs-sync-config.sh` | toolu | dependency | shell-out | supported | #210/todo | native-bash | yes | — |
| `toolu:lib:dependency:edit-records.sh` | `plugins/toolu/hooks/lib/edit-records.sh` | toolu | dependency | shell-out | supported | #210/todo | native-bash | yes | — |
| `toolu:lib:dependency:gate-file.sh` | `plugins/toolu/hooks/lib/gate-file.sh` | toolu | dependency | shell-out | supported | #210/todo | native-bash | yes | — |
| `toolu:lib:dependency:gate-mode.sh` | `plugins/toolu/hooks/lib/gate-mode.sh` | toolu | dependency | shell-out | supported | #210/todo | native-bash | yes | — |
| `toolu:lib:dependency:host.sh` | `plugins/toolu/hooks/lib/host.sh` | toolu | dependency | shell-out | supported | #210/todo | native-bash | yes | — |
| `toolu:lib:dependency:permissions.sh` | `plugins/toolu/hooks/lib/permissions.sh` | toolu | dependency | no-map | n/a | n/a | n/a | yes | Host-specific helper/surface; not an OpenCode enforcement target. |
| `toolu:lib:dependency:plan-ledger-parse.sh` | `plugins/toolu/hooks/lib/plan-ledger-parse.sh` | toolu | dependency | shell-out | supported | #210/todo | native-bash | yes | — |
| `toolu:lib:dependency:plan-ledger-preflight.sh` | `plugins/toolu/hooks/lib/plan-ledger-preflight.sh` | toolu | dependency | shell-out | supported | #210/todo | native-bash | yes | — |
| `toolu:lib:dependency:plan-ledger.sh` | `plugins/toolu/hooks/lib/plan-ledger.sh` | toolu | dependency | shell-out | supported | #210/todo | native-bash | yes | — |
| `toolu:lib:dependency:push-waiver.sh` | `plugins/toolu/hooks/lib/push-waiver.sh` | toolu | dependency | shell-out | supported | #210/todo | native-bash | yes | — |
| `toolu:lib:dependency:quality-config.sh` | `plugins/toolu/hooks/lib/quality-config.sh` | toolu | dependency | shell-out | supported | #210/todo | native-bash | yes | — |
| `toolu:lib:dependency:registry.sh` | `plugins/toolu/hooks/lib/registry.sh` | toolu | dependency | shell-out | supported | #210/todo | native-bash | yes | — |
| `toolu:lib:dependency:state-sweeper.sh` | `plugins/toolu/hooks/lib/state-sweeper.sh` | toolu | dependency | shell-out | supported | #210/todo | native-bash | yes | — |
| `toolu:lib:dependency:telemetry.sh` | `plugins/toolu/hooks/lib/telemetry.sh` | toolu | dependency | shell-out | supported | #210/todo | native-bash | yes | — |
| `toolu:lib:dependency:verdict.sh` | `plugins/toolu/hooks/lib/verdict.sh` | toolu | dependency | shell-out | supported | #210/todo | native-bash | yes | — |
| `ts-quality:hooks.json:SessionStart:check-toolu.js:startup|resume|clear|compact` | `plugins/ts-quality/hooks/hooks.json` | ts-quality | SessionStart | shell-out | required | #204/todo | pending-opencode | no | — |
| `ts-quality:hooks.json:SessionStart:register.js:startup|resume|clear|compact` | `plugins/ts-quality/hooks/hooks.json` | ts-quality | SessionStart | shell-out | required | #204/todo | pending-opencode | no | — |

## Notes

- Concern fragments are inventoried with `parentId` pointing at `register.sh` when present; they are not independently executable.
- `hostMechanism=pending-opencode` is resolved against OpenCode pins during #204/#212.
- Verification conformance links are filled by #212.
