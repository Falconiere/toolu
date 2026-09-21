# Gate coverage matrix

**Issue:** [#209](https://github.com/Falconiere/toolu/issues/209) (epic [#203](https://github.com/Falconiere/toolu/issues/203))  
**Inventory:** `tooling/fixtures/gate-coverage/inventory.json`  
**Check:** `bun run tooling/src/gate-coverage-inventory.ts check`

Classifications match [docs/portable-core.md](portable-core.md): `shell-out` · `port-native` · `port-new` · `no-map`.

| id | source | plugin | event | classification | support | impl | host mechanism | bash | limits |
|----|--------|--------|-------|----------------|---------|------|----------------|------|--------|
| `agent-browser:entrypoint:SessionStart:session-start.sh` | `plugins/agent-browser/hooks/session-start.sh` | agent-browser | SessionStart | shell-out | required | #211/todo | pending-opencode | yes | — |
| `agent-browser:hooks.json:SessionStart:session-start.sh:startup|resume|clear|compact` | `plugins/agent-browser/hooks/hooks.json` | agent-browser | SessionStart | shell-out | required | #210/todo | pending-opencode | yes | — |
| `ast-grep:entrypoint:SessionStart:register.sh` | `plugins/ast-grep/hooks/register.sh` | ast-grep | SessionStart | shell-out | required | #210/todo | pending-opencode | yes | — |
| `ast-grep:hooks.json:SessionStart:register.sh:startup|resume|clear|compact` | `plugins/ast-grep/hooks/hooks.json` | ast-grep | SessionStart | shell-out | required | #210/todo | pending-opencode | yes | — |
| `ast-grep:post-tools.d:PostToolUse:byte-savings.sh` | `plugins/ast-grep/hooks/post-tools.d/byte-savings.sh` | ast-grep | PostToolUse | shell-out | required | #210/todo | pending-opencode | yes | — |
| `ast-grep:pre-tools.d:PreToolUse:search-nudge.sh` | `plugins/ast-grep/hooks/pre-tools.d/search-nudge.sh` | ast-grep | PreToolUse | shell-out | required | #210/todo | pending-opencode | yes | — |
| `context7:entrypoint:SessionStart:session-start.sh` | `plugins/context7/hooks/session-start.sh` | context7 | SessionStart | shell-out | required | #211/todo | pending-opencode | yes | — |
| `context7:hooks.json:SessionStart:session-start.sh:startup|resume|clear|compact` | `plugins/context7/hooks/hooks.json` | context7 | SessionStart | shell-out | required | #210/todo | pending-opencode | yes | — |
| `exa-search:entrypoint:SessionStart:session-start.sh` | `plugins/exa-search/hooks/session-start.sh` | exa-search | SessionStart | shell-out | required | #211/todo | pending-opencode | yes | — |
| `exa-search:hooks.json:SessionStart:session-start.sh:startup|resume|clear|compact` | `plugins/exa-search/hooks/hooks.json` | exa-search | SessionStart | shell-out | required | #210/todo | pending-opencode | yes | — |
| `jev:entrypoint:SessionStart:session-start.sh` | `plugins/jev/hooks/session-start.sh` | jev | SessionStart | shell-out | required | #211/todo | pending-opencode | yes | — |
| `jev:entrypoint:UserPromptSubmit:user-prompt-submit.sh` | `plugins/jev/hooks/user-prompt-submit.sh` | jev | UserPromptSubmit | shell-out | required | #210/todo | pending-opencode | yes | — |
| `jev:hooks.json:SessionStart:session-start.sh:startup|resume|clear|compact` | `plugins/jev/hooks/hooks.json` | jev | SessionStart | shell-out | required | #210/todo | pending-opencode | yes | — |
| `jev:hooks.json:UserPromptSubmit:user-prompt-submit.sh` | `plugins/jev/hooks/hooks.json` | jev | UserPromptSubmit | shell-out | required | #210/todo | pending-opencode | yes | — |
| `jira:entrypoint:SessionStart:session-start.sh` | `plugins/jira/hooks/session-start.sh` | jira | SessionStart | shell-out | required | #211/todo | pending-opencode | yes | — |
| `jira:hooks.json:SessionStart:session-start.sh:startup|resume|clear|compact` | `plugins/jira/hooks/hooks.json` | jira | SessionStart | shell-out | required | #210/todo | pending-opencode | yes | — |
| `pr-babysit:entrypoint:SessionStart:check-toolu.sh` | `plugins/pr-babysit/hooks/check-toolu.sh` | pr-babysit | SessionStart | no-map | n/a | n/a | n/a | yes | Host-specific helper/surface; not an OpenCode enforcement target. |
| `pr-babysit:hooks.json:SessionStart:check-toolu.sh:startup|resume|clear|compact` | `plugins/pr-babysit/hooks/hooks.json` | pr-babysit | SessionStart | shell-out | required | #210/todo | pending-opencode | yes | — |
| `python-quality:concern:PostToolUse:00-preamble.sh` | `plugins/python-quality/hooks/concerns/00-preamble.sh` | python-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `python-quality:concern:PostToolUse:10-size-file.sh` | `plugins/python-quality/hooks/concerns/10-size-file.sh` | python-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `python-quality:concern:PostToolUse:20-tests.sh` | `plugins/python-quality/hooks/concerns/20-tests.sh` | python-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `python-quality:concern:PostToolUse:30-suppression.sh` | `plugins/python-quality/hooks/concerns/30-suppression.sh` | python-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `python-quality:concern:PostToolUse:50-size-fn.sh` | `plugins/python-quality/hooks/concerns/50-size-fn.sh` | python-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `python-quality:concern:PostToolUse:70-no-mocks.sh` | `plugins/python-quality/hooks/concerns/70-no-mocks.sh` | python-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `python-quality:concern:PostToolUse:90-docs.sh` | `plugins/python-quality/hooks/concerns/90-docs.sh` | python-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `python-quality:concern:PostToolUse:99-finalize.sh` | `plugins/python-quality/hooks/concerns/99-finalize.sh` | python-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `python-quality:entrypoint:SessionStart:check-toolu.sh` | `plugins/python-quality/hooks/check-toolu.sh` | python-quality | SessionStart | shell-out | required | #204/todo | pending-opencode | yes | — |
| `python-quality:entrypoint:SessionStart:register.sh` | `plugins/python-quality/hooks/register.sh` | python-quality | SessionStart | shell-out | required | #204/todo | pending-opencode | yes | — |
| `python-quality:hooks.json:SessionStart:check-toolu.sh:startup|resume|clear|compact` | `plugins/python-quality/hooks/hooks.json` | python-quality | SessionStart | shell-out | required | #204/todo | pending-opencode | yes | — |
| `python-quality:hooks.json:SessionStart:register.sh:startup|resume|clear|compact` | `plugins/python-quality/hooks/hooks.json` | python-quality | SessionStart | shell-out | required | #204/todo | pending-opencode | yes | — |
| `rust-quality:concern:PostToolUse:00-preamble.sh` | `plugins/rust-quality/hooks/concerns/00-preamble.sh` | rust-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `rust-quality:concern:PostToolUse:10-size-file.sh` | `plugins/rust-quality/hooks/concerns/10-size-file.sh` | rust-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `rust-quality:concern:PostToolUse:20-tests.sh` | `plugins/rust-quality/hooks/concerns/20-tests.sh` | rust-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `rust-quality:concern:PostToolUse:30-suppression.sh` | `plugins/rust-quality/hooks/concerns/30-suppression.sh` | rust-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `rust-quality:concern:PostToolUse:40-unsafe.sh` | `plugins/rust-quality/hooks/concerns/40-unsafe.sh` | rust-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `rust-quality:concern:PostToolUse:50-size-fn.sh` | `plugins/rust-quality/hooks/concerns/50-size-fn.sh` | rust-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `rust-quality:concern:PostToolUse:55-size-impl.sh` | `plugins/rust-quality/hooks/concerns/55-size-impl.sh` | rust-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `rust-quality:concern:PostToolUse:60-error-handling.sh` | `plugins/rust-quality/hooks/concerns/60-error-handling.sh` | rust-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `rust-quality:concern:PostToolUse:70-no-mocks.sh` | `plugins/rust-quality/hooks/concerns/70-no-mocks.sh` | rust-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `rust-quality:concern:PostToolUse:90-docs.sh` | `plugins/rust-quality/hooks/concerns/90-docs.sh` | rust-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `rust-quality:concern:PostToolUse:99-finalize.sh` | `plugins/rust-quality/hooks/concerns/99-finalize.sh` | rust-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `rust-quality:entrypoint:SessionStart:check-toolu.sh` | `plugins/rust-quality/hooks/check-toolu.sh` | rust-quality | SessionStart | shell-out | required | #204/todo | pending-opencode | yes | — |
| `rust-quality:entrypoint:SessionStart:register.sh` | `plugins/rust-quality/hooks/register.sh` | rust-quality | SessionStart | shell-out | required | #204/todo | pending-opencode | yes | — |
| `rust-quality:hooks.json:SessionStart:check-toolu.sh:startup|resume|clear|compact` | `plugins/rust-quality/hooks/hooks.json` | rust-quality | SessionStart | shell-out | required | #204/todo | pending-opencode | yes | — |
| `rust-quality:hooks.json:SessionStart:register.sh:startup|resume|clear|compact` | `plugins/rust-quality/hooks/hooks.json` | rust-quality | SessionStart | shell-out | required | #204/todo | pending-opencode | yes | — |
| `statusline:entrypoint:SessionStart:session-start.sh` | `plugins/statusline/hooks/session-start.sh` | statusline | SessionStart | shell-out | required | #211/todo | pending-opencode | yes | — |
| `statusline:hooks.json:SessionStart:session-start.sh:startup|resume|clear|compact` | `plugins/statusline/hooks/hooks.json` | statusline | SessionStart | shell-out | required | #210/todo | pending-opencode | yes | — |
| `toolu-review:entrypoint:SessionStart:session-start.sh` | `plugins/toolu-review/hooks/session-start.sh` | toolu-review | SessionStart | shell-out | required | #211/todo | pending-opencode | yes | — |
| `toolu-review:hooks.json:SessionStart:session-start.sh:startup|resume|clear|compact` | `plugins/toolu-review/hooks/hooks.json` | toolu-review | SessionStart | shell-out | required | #210/todo | pending-opencode | yes | — |
| `toolu:builtin-module:PostToolUse:gate-status.sh` | `plugins/toolu/hooks/post-tools/modules/gate-status.sh` | toolu | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `toolu:builtin-module:PostToolUse:push-waiver.sh` | `plugins/toolu/hooks/post-tools/modules/push-waiver.sh` | toolu | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `toolu:builtin-module:PreToolUse:bash-commands.sh` | `plugins/toolu/hooks/pre-tools/modules/bash-commands.sh` | toolu | PreToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `toolu:builtin-module:PreToolUse:code-edit-rules.sh` | `plugins/toolu/hooks/pre-tools/modules/code-edit-rules.sh` | toolu | PreToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `toolu:builtin-module:PreToolUse:commit-gate.sh` | `plugins/toolu/hooks/pre-tools/modules/commit-gate.sh` | toolu | PreToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `toolu:builtin-module:PreToolUse:docs-sync.sh` | `plugins/toolu/hooks/pre-tools/modules/docs-sync.sh` | toolu | PreToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `toolu:builtin-module:PreToolUse:mcp-blocker.sh` | `plugins/toolu/hooks/pre-tools/modules/mcp-blocker.sh` | toolu | PreToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `toolu:builtin-module:PreToolUse:plan-ledger.sh` | `plugins/toolu/hooks/pre-tools/modules/plan-ledger.sh` | toolu | PreToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `toolu:builtin-module:PreToolUse:protected-files.sh` | `plugins/toolu/hooks/pre-tools/modules/protected-files.sh` | toolu | PreToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `toolu:builtin-module:PreToolUse:push-review.sh` | `plugins/toolu/hooks/pre-tools/modules/push-review.sh` | toolu | PreToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `toolu:builtin-module:PreToolUse:quality-gate.sh` | `plugins/toolu/hooks/pre-tools/modules/quality-gate.sh` | toolu | PreToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `toolu:entrypoint:PreCompact:pre-compact.sh` | `plugins/toolu/hooks/pre-compact.sh` | toolu | PreCompact | shell-out | required | #210/todo | pending-opencode | yes | — |
| `toolu:entrypoint:PreToolUse:agent-tier.sh` | `plugins/toolu/hooks/pre-tools/agent-tier.sh` | toolu | PreToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `toolu:entrypoint:SessionStart:session-start.sh` | `plugins/toolu/hooks/session-start.sh` | toolu | SessionStart | shell-out | required | #211/todo | pending-opencode | yes | — |
| `toolu:entrypoint:UserPromptSubmit:user-prompt-submit.sh` | `plugins/toolu/hooks/user-prompt-submit.sh` | toolu | UserPromptSubmit | shell-out | required | #210/todo | pending-opencode | yes | — |
| `toolu:hooks.json:PostToolUse:mod.sh:apply_patch|Edit|Write|MultiEdit|Bash|Sh` | `plugins/toolu/hooks/hooks.json` | toolu | PostToolUse | shell-out | required | #210/todo | pending-opencode | yes | — |
| `toolu:hooks.json:PreCompact:pre-compact.sh:auto` | `plugins/toolu/hooks/hooks.json` | toolu | PreCompact | shell-out | required | #210/todo | pending-opencode | yes | — |
| `toolu:hooks.json:PreToolUse:agent-tier.sh:spawn_agent|Agent|Task` | `plugins/toolu/hooks/hooks.json` | toolu | PreToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `toolu:hooks.json:PreToolUse:mcp-blocker.sh:mcp__` | `plugins/toolu/hooks/hooks.json` | toolu | PreToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `toolu:hooks.json:PreToolUse:mod.sh:apply_patch|Edit|Write|MultiEdit|Bash|Sh` | `plugins/toolu/hooks/hooks.json` | toolu | PreToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `toolu:hooks.json:SessionStart:session-start.sh:startup|resume|clear|compact` | `plugins/toolu/hooks/hooks.json` | toolu | SessionStart | shell-out | required | #210/todo | pending-opencode | yes | — |
| `toolu:hooks.json:UserPromptSubmit:user-prompt-submit.sh` | `plugins/toolu/hooks/hooks.json` | toolu | UserPromptSubmit | shell-out | required | #210/todo | pending-opencode | yes | — |
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
| `ts-quality:concern:PostToolUse:00-preamble.sh` | `plugins/ts-quality/hooks/concerns/00-preamble.sh` | ts-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:concern:PostToolUse:10-imports.sh` | `plugins/ts-quality/hooks/concerns/10-imports.sh` | ts-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:concern:PostToolUse:15-type-as.sh` | `plugins/ts-quality/hooks/concerns/15-type-as.sh` | ts-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:concern:PostToolUse:20-tests.sh` | `plugins/ts-quality/hooks/concerns/20-tests.sh` | ts-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:concern:PostToolUse:25-size-file.sh` | `plugins/ts-quality/hooks/concerns/25-size-file.sh` | ts-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:concern:PostToolUse:30-size-fn.sh` | `plugins/ts-quality/hooks/concerns/30-size-fn.sh` | ts-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:concern:PostToolUse:35-react-hooks.sh` | `plugins/ts-quality/hooks/concerns/35-react-hooks.sh` | ts-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:concern:PostToolUse:40-factory.sh` | `plugins/ts-quality/hooks/concerns/40-factory.sh` | ts-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:concern:PostToolUse:45-typeguard.sh` | `plugins/ts-quality/hooks/concerns/45-typeguard.sh` | ts-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:concern:PostToolUse:50-type-dup.sh` | `plugins/ts-quality/hooks/concerns/50-type-dup.sh` | ts-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:concern:PostToolUse:55-naming.sh` | `plugins/ts-quality/hooks/concerns/55-naming.sh` | ts-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:concern:PostToolUse:60-console.sh` | `plugins/ts-quality/hooks/concerns/60-console.sh` | ts-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:concern:PostToolUse:65-suppression.sh` | `plugins/ts-quality/hooks/concerns/65-suppression.sh` | ts-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:concern:PostToolUse:70-ui-confirm.sh` | `plugins/ts-quality/hooks/concerns/70-ui-confirm.sh` | ts-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:concern:PostToolUse:72-ui-radix.sh` | `plugins/ts-quality/hooks/concerns/72-ui-radix.sh` | ts-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:concern:PostToolUse:74-react-props.sh` | `plugins/ts-quality/hooks/concerns/74-react-props.sh` | ts-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:concern:PostToolUse:76-toast.sh` | `plugins/ts-quality/hooks/concerns/76-toast.sh` | ts-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:concern:PostToolUse:78-error-ast.sh` | `plugins/ts-quality/hooks/concerns/78-error-ast.sh` | ts-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:concern:PostToolUse:80-throw-literal.sh` | `plugins/ts-quality/hooks/concerns/80-throw-literal.sh` | ts-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:concern:PostToolUse:85-no-mocks.sh` | `plugins/ts-quality/hooks/concerns/85-no-mocks.sh` | ts-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:concern:PostToolUse:90-duplication.sh` | `plugins/ts-quality/hooks/concerns/90-duplication.sh` | ts-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:concern:PostToolUse:92-docs.sh` | `plugins/ts-quality/hooks/concerns/92-docs.sh` | ts-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:concern:PostToolUse:94-handler.sh` | `plugins/ts-quality/hooks/concerns/94-handler.sh` | ts-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:concern:PostToolUse:99-finalize.sh` | `plugins/ts-quality/hooks/concerns/99-finalize.sh` | ts-quality | PostToolUse | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:entrypoint:SessionStart:check-toolu.sh` | `plugins/ts-quality/hooks/check-toolu.sh` | ts-quality | SessionStart | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:entrypoint:SessionStart:register.sh` | `plugins/ts-quality/hooks/register.sh` | ts-quality | SessionStart | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:hooks.json:SessionStart:check-toolu.sh:startup|resume|clear|compact` | `plugins/ts-quality/hooks/hooks.json` | ts-quality | SessionStart | shell-out | required | #204/todo | pending-opencode | yes | — |
| `ts-quality:hooks.json:SessionStart:register.sh:startup|resume|clear|compact` | `plugins/ts-quality/hooks/hooks.json` | ts-quality | SessionStart | shell-out | required | #204/todo | pending-opencode | yes | — |

## Notes

- Concern fragments are inventoried with `parentId` pointing at `register.sh` when present; they are not independently executable.
- `hostMechanism=pending-opencode` is resolved against OpenCode pins during #204/#212.
- Verification conformance links are filled by #212.
