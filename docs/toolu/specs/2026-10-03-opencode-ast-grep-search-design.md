# OpenCode ast-grep search enforcement and savings — Design

**Date:** 2026-10-03 **Status:** Approved **Author:** Claude Code **Topic:** OP-13 (#347): ast-grep search-nudge, byte-savings report and skill on the pinned OpenCode host

## Problem

The OpenCode bootstrap registers ast-grep's two registry modules, and the OP-04/OP-05/OP-06 bridges now run `pre-tools.d` and `post-tools.d` modules. Nothing proves the modules take the right paths on OpenCode's own tools (`grep`, `bash`, `read`, `glob`), and the post side is broken in two ways.

1. The post bridge sends the tool's text as `tool_output` and only host metadata as `tool_response`. byte-savings measures `tool_response`, so every OpenCode record is the size of a metadata JSON object (46 bytes for `{"metadata":{"matches":1},"interrupted":false}`), not the bytes the tool put into context.
2. The module never speaks. The OP-01 host contract (`capability-matrix.json`, `ast-grep.postTool`: "byte-savings reports search output savings after the tool runs", `tool.execute.after (append to output)`, supported) and the issue ("their context is visible to the model") both require a model-visible savings report on OpenCode.

The generated `ast-grep-ast-grep` skill has never been loaded by the host. Its rewrite workflow also points at `hooks/lib/detect.sh` (`detect_node_pm` / `detect_rust`), a bash library removed in the TypeScript migration, so that instruction is dead on every host.

## Non-Goals

1. Change nudge wording, rules or the `skills.ast-grep` opt-out. They stay byte-identical on every host.
2. Change Claude Code or Codex behavior. byte-savings stays silent there, and its golden corpus is unchanged.
3. Claim a native pre-tool advisory channel. The pinned host has none (`pre.advisory`). OP-05's advice store delivers the nudge with the tool result, and the matrix keeps `ast-grep.tools` as `unsupported` with that alternative.
4. Rework the core dispatcher, the adapter's argument validation, OP-27's packaging, or the CI wiring of live smokes (OP-28, #362).

## Architecture

**Routing.** Reuse OP-03 to OP-06. The adapter maps OpenCode `grep` to `Grep` (`include` doubles as `glob`), `bash` to `Bash`, `read` to `Read` and `glob` to `Glob`. search-nudge's rules already take the correct paths for these inputs, and its advisory reaches the model through `tool.execute.after` via the advice store.

**Measurement fix (adapter).** The post payload's `tool_response` also carries the host's text as `output`. byte-savings already reads `.content // .stdout // .output`, as it does on other hosts. `tool-exit.ts` reads `tool_response.metadata.exit_code` and `interrupted` first, so shell outcomes are unchanged. The bridge builds the payload from `output.output` before it appends post diagnostics, and `advice.after` runs last, so toolu's own text is never measured. Registry modules already see `tool_response` as `event.toolOutput`, so the text becomes visible there too. The text appears twice in one in-process payload; that is accepted, because the host already bounds tool output.

**Visible report (module, OpenCode only).** Decision: Jev `visible` (0.99) once the contract row and issue text were weighed. The earlier `silent` (0.86) was made without them. Frequency: `astgrep` (0.64). When `ctx.host === "opencode"` and the recorded call is an `ast-grep` run, byte-savings returns an advisory with the session's report: one line per kind, then the total. That is the same text `byte-savings-report.js` prints, from one shared function moved to `lib/savings-report.ts`. Read, grep and glob results get no extra text, so the report costs context only where it shows the comparison, after a structural search. Every other host gets `allow`, as today.

**Skill.** Replace the dead `detect.sh` pointer with host-neutral wording ("the project's own typecheck and lint commands") and regenerate the mirrors.

**Proof.** A hermetic adapter test publishes the modules with the real `register.js` under the OpenCode process env and drives the real before/after handlers. A pinned-host smoke (`opencode-ai@1.18.34`, scripted loopback provider, isolated profile) does the rest. It loads the skill with the native `skill` tool, runs the nudged and plain searches, runs the skill's search example against a real project, and reads the ledger the session wrote.

## Interfaces / Schema

- `tool-post.ts` payload: `{ ...request, tool_output: <text>, tool_response: { metadata, interrupted: false, output: <text> } }`. Shell metadata stays `{ exit_code }`.
- `plugins/ast-grep/hooks/src/lib/savings-report.ts`: `parseLedger(text): LedgerRecord[] | { line: number }` and `report(records): string`. Both `byte-savings-report.ts` (unchanged output) and `byte-savings.ts` use them.
- byte-savings decision: `{ kind: "advisory", message: "ast-grep byte savings this session:\n<report>" }` when host is `opencode`, kind is `ast-grep`, and the record was appended. Otherwise `{ kind: "allow" }`. The post bridge shows it as `[toolu post-check after execution]\n<message>`.
- Ledger: `$TOOLU_CONFIG_DIR/toolu/byte-savings/<session id, [A-Za-z0-9-] only>.jsonl`. `TOOLU_CONFIG_DIR` is the project's data root: `.opencode/toolu/state` by default, or `<override>/toolu/opencode/projects/<name>-<hash>` under an override. Gates and bash see the same value.
- On-demand report from the session's bash, documented: `bun "$TOOLU_PLUGIN_ROOT_AST_GREP/hooks/dist/byte-savings-report.js" "$TOOLU_CONFIG_DIR"/toolu/byte-savings/<file>.jsonl`. `shellEnvFor` exports `TOOLU_PLUGIN_ROOT_AST_GREP` and `TOOLU_CONFIG_DIR`. The ledger file name is the sanitized session id, so the docs list the directory.
- Live smoke: `bun run smoke:opencode-ast-grep` (`tooling/src/opencode-ast-grep-smoke.ts`, scenarios in `tooling/src/opencode-host/scenarios-ast-grep-smoke.ts`). It exports `prepareSdk` from `scenarios-posttool-smoke.ts` and passes `.opencode/toolu/plugins.json` `enabled: ["toolu","ast-grep"]` over `session()`'s `["toolu"]` default. It fails fast with a named error when `ast-grep` is not on `PATH`.

## Failure modes and edge cases

- **Empty text:** `returnedBytes` is undefined, so there is no record and no report (same as other hosts).
- **Non-string output:** the bridge reports a post-check failure and dispatches nothing.
- **Truncated output:** a result the host truncated (`metadata.truncated`) records the truncated bytes, which is what entered context. This is intended.
- **Duplicate after:** a second after for one `(sessionID, callID)` hits the completion guard. There is one ledger line, one report and one nudge.
- **Thrown or denied tool:** there is no after hook, so nothing is recorded. Its pending nudge is never delivered and expires by TTL or the next `begin` for that key.
- **Interrupted or unconfirmed call:** `tool-post.ts` returns before dispatch for an interrupted call, or a shell call without a confirmed non-negative integer `metadata.exit`. Nothing is recorded or reported for it, including an interrupted `ast-grep run`.
- **Empty `sessionID`/`callID`:** `mapToolCall` denies the call before execution, so nothing is recorded.
- **Concurrency:** parallel calls in one session each append one short line with `appendFileSync`. The report counts whatever lines exist when it runs. Two sessions write two files.
- **Unreadable or corrupt ledger during the report:** the record is still appended, and the advisory says the report is unavailable rather than throwing. An unwritable data root records nothing and reports nothing (existing best-effort behavior).
- **Searches:** piped `grep`/`rg` (`git log | grep x`), `grep` with a non-code `include` (`*.md`), and the `grep` tool with a literal pattern (`TODO`) get no nudge. A bash `grep`/`rg` over files with a literal pattern (`rg TODO notes.md`) gets the generic nudge.
- **ast-grep missing from `PATH`:** WARN nudge text. **Opt-out** (`skills.ast-grep: false` in `.opencode/toolu.config.json`): structural nudges are dropped and the generic one loses its ast-grep half. Both are existing module behavior, re-proven here.
- **Read ratio:** OpenCode's `read` output carries line numbers and wrapper tags, so `returned` can exceed the file's `full` size. The report's `saved=` percentage is computed as-is, and the docs note the difference.

## Acceptance criteria

- **AC-1:** Given ast-grep published by its real `register.js`, these OpenCode calls each get exactly one `[toolu advisory]` with the structural STOP text: `grep` with `export function greet`, and `bash` `rg 'function greet' src`. `bash` `rg TODO notes.md` gets the generic text. `grep` with `include: "*.md"`, `grep` with `TODO`, and `bash` `git log | grep init` get none. Without ast-grep on `PATH`, the structural calls get the WARN text. With the opt-out, `grep` structural gets none.
- **AC-2:** Given completed `read`, `grep`, `glob` and `bash ast-grep run …` calls, the ledger holds exactly one line per call. `returned` equals the UTF-8 byte length of the host's result text without trailing newlines; toolu's advice on a nudged call is excluded. `read` records the file size as `full`. A duplicate after and N parallel after calls add exactly one line per distinct call. `bash rg …` and an interrupted `ast-grep run` add none.
- **AC-3:** On OpenCode, the `bash ast-grep run …` result ends with one `[toolu post-check after execution]` block that holds the session report, with an `ast-grep:` line and a `TOTAL returned:` line. `read`/`grep`/`glob` results get no savings text. The same module under host `claude` returns `allow`, and the golden corpus stays unchanged.
- **AC-4:** The pinned host loads `ast-grep-ast-grep` with its native `skill` tool, and the text it returns contains no `detect.sh` reference. The skill's search example (`ast-grep run -p '<pattern>' -l typescript <path>`) with a concrete pattern, run from the session's bash in a real git project, returns the matching function.
- **AC-5:** In that pinned-host session, the AC-1 nudges and the AC-3 report reach the model in the tool messages, and the ledger under the session's data root matches AC-2.
- **AC-6:** Claude Code and Codex behavior is unchanged: ast-grep's golden corpus, the `byte-savings-report` tests and the existing OpenCode post-tool tests pass, and `bun run test` is green.

## Acceptance evidence

| AC   | Real input and expected result | Boundary and runnable check |
| ---- | ------------------------------ | --------------------------- |
| AC-1 | Temp git project and data root. Real `register.js` run under `tooluProcessEnv`. Real before/after handlers with OpenCode call shapes; read the after output. | PATH without ast-grep; opt-out config. `bun test tools/toolu-opencode/src/adapter/__tests__/ast-grep-modules.test.ts`. Existing `evaluate.test.ts`/`tool-advice.test.ts` call search-nudge directly; the new test covers register-published modules on OpenCode tool shapes and the post side. |
| AC-2 | Same setup with a real file for `read` and host metadata (`matches`, `exit`); read the ledger JSONL. | Duplicate after, 8 parallel after calls, empty output, nudged call. Same test file. |
| AC-3 | Same setup; `bash` `ast-grep run -p 'export function $N($$$) { $$$ }' -l typescript src` with its real stdout as host output. | Host `claude` returns allow (module test); corrupt ledger. Same test file plus `bun test plugins/ast-grep/hooks/src/__tests__/` |
| AC-4 | Pinned host, isolated profile. Scripted `skill {name:"ast-grep-ast-grep"}`, then the example through `bash` on `src/app.ts`. | ast-grep absent from `PATH`: the smoke fails with a named error. `bun run smoke:opencode-ast-grep` |
| AC-5 | The same session's tool messages and `$TOOLU_CONFIG_DIR/toolu/byte-savings/*.jsonl` under its default data root. | Calls without a nudge show no advisory. `bun run smoke:opencode-ast-grep`. Its output is recorded in the PR; the gating CI run belongs to OP-28 (#362), like the other OpenCode smokes. |
| AC-6 | Existing suites. | `bun run test` |

## Documentation impact

- `docs/opencode.md`: an ast-grep section (nudge with the tool result, the ledger location, the report on ast-grep results, the on-demand command, the read-ratio note) and the `smoke:opencode-ast-grep` entry next to the other smokes.
- `docs/opencode-host-contract.md` and `docs/portable-core.md`: name the new smoke where the other smokes are listed.
- `capability-matrix.json`: keep `ast-grep.postTool` as is (now true); refine `ast-grep.tools.alternative` only if the contract check requires it.
- `plugins/ast-grep/README.md`: one OpenCode line.
- `plugins/ast-grep/skills/ast-grep/SKILL.md`: replace the dead `detect.sh` pointer. The `comemory.sh` line stays: it applies only when that plugin is installed, and it is outside this scope.
- `plugins/ast-grep/hooks/src/byte-savings.ts` header comment ("never speaks") and the README wording: update to say it reports on OpenCode.
- Bundles: `bun run build:plugins`, then commit `plugins/ast-grep/hooks/dist/byte-savings.js` and `byte-savings-report.js` (`check:plugin-bundles`).
- `package.json`: add the `smoke:opencode-ast-grep` script.
- Regenerate the mirrors (`bun run generate:opencode-surface`) and check them (`check:opencode-surface`, `test:portable-core`).

## Open Questions

None blocking. The visible-report decision follows the approved OP-01 contract row and the issue's acceptance text; the reasoning is recorded above. The orchestrator can overrule it in review without changing the measurement fix. CI gating of the live smoke is owned by OP-28 (#362).
