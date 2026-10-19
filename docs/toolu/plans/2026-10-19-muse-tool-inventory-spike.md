# Muse tool inventory and hook-verdict spike — Plan

**Date:** 2026-10-19   **Status:** Draft   **Spec:** docs/toolu/specs/2026-10-19-muse-tool-inventory-spike-design.md   **Topic:** Issue #540, observed Muse tool inventory and the ask, input-rewrite and PostToolUse-block verdicts

## Evidence and approach

Recalled the OpenCode OP-01 live-probe precedent: a scripted headless host, with evidence JSON committed next to a contract doc.

Inspected:

- the epic's own probe sessions under `~/.local/share/muse/sessions/2026/10/19`;
- the Muse native plugin contract (`muse-core/create-plugin/references/native-plugin-contract.md`);
- `docs/research/` (the research report precedent) and `docs/toolu/evidence/` (the evidence precedent);
- `docs/markdown-cli.md`, which shows that `text` fences are skipped and `docs/toolu/**` is not scanned.

Verified before planning:

- The probe plugin validates clean.
- `XDG_DATA_HOME=<scratch>/xdg-data` isolates the plugin store, while `--scope project` does not.
- Approval trust is written to `~/.config/muse/settings.json` `runtime_capabilities`.
- A smoke session logged `bash` (`command`, `description`, `workdir`) and the internal `submit_reminder_decision`.
- Its toolset shows the MCP tool as `mcp__plugin_toolu_probe_540_probe_mcp__probe_echo`.

Every run uses `MUSE_UPDATE_INTERVAL_SECONDS=99999999`, `XDG_DATA_HOME=<scratch>/xdg-data`, cwd `<scratch>/ws` and `muse exec --json`.

The probe, the run logs and the extractor (`extract.py`) stay in the scratchpad. The ledger checks read only committed artifacts, live Muse state and the issue, so `toolu ledger run --verify` stays reproducible.

## Workstream summary

inventory runs → PermissionRequest runs → verdict experiments → evidence JSON → research doc → cleanup → issue comment → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "S1",
    "title": "Build, validate (muse plugins validate, zero diagnostics), install and approve the toolu-probe-540 probe in the sandbox data home; take the before snapshot of global Muse state (snap.py take); run I0-I9, P1, S1 (spec run matrix); cross-check model calls against hook lines; write evidence muse/toolsets/tools (families, configs, exercised, tool_input_keys, post_event, tool_response_shape, redacted pre/post examples); git add -f the evidence",
    "check": "jq -e '(.muse.version | startswith(\"1.4.10\")) and ((.toolsets | keys) - [\"default-untrusted\", \"default-trusted\", \"native-basic\", \"enable-shell-tool\"] | length == 0) and ([.toolsets[] | length > 0] | all) and ([.toolsets[][]] - (.tools | keys) | length == 0) and ([.tools[] | select(.internal == false) | if .exercised then (.tool_input_keys | length > 0) and (.post_event | IN(\"PostToolUse\", \"PostToolUseFailure\")) and (.pre_example | type == \"object\" and length > 0) and (.tool_response_shape | type == \"string\" and length > 0) else (.not_exercised_reason | type == \"string\" and length > 0) end] | all) and ([\"shell\", \"file-read\", \"file-write\", \"file-edit\", \"search\", \"web\", \"skill\", \"memory\", \"cron\", \"delegation\", \"mcp\"] - [.tools[] | select(.exercised == true) | .family] | length == 0)' docs/toolu/evidence/2026-10-19-muse-tool-inventory.json",
    "ac_refs": [
      "AC-1",
      "AC-7"
    ],
    "paths": [
      "docs/toolu/evidence/2026-10-19-muse-tool-inventory.json"
    ],
    "input": "live muse exec 1.4.10 sessions (runs I0-I9, P1, S1) with the toolu-probe-540 logging hooks; logs/hooks.jsonl and the sandbox session.jsonl files",
    "model": "inherit"
  },
  {
    "id": "S2",
    "title": "Internal-tool ignore list in evidence: each internal tool absent from the main toolsets with its reminder/child-agent origin cited",
    "check": "jq -e '. as $r | [.tools | to_entries[] | select(.value.internal == true)] as $int | ($int | length > 0) and ([$int[] | (.value.origin | type == \"string\" and length > 0) and (.key as $k | [$r.toolsets[][]] | index($k) | not)] | all)' docs/toolu/evidence/2026-10-19-muse-tool-inventory.json",
    "ac_refs": [
      "AC-3"
    ],
    "depends_on": [
      "S1"
    ],
    "paths": [
      "docs/toolu/evidence/2026-10-19-muse-tool-inventory.json"
    ],
    "input": "reminder-agent subagent session logs (toolset mode named, source runtime) from the S1 runs",
    "model": "inherit"
  },
  {
    "id": "S3",
    "title": "PermissionRequest runs A1-A9, AP1 and AS1 under --approval-mode untrusted --trust-workspace; record permission_request per tool and the PermissionRequest payload keys",
    "check": "jq -e '([.tools[] | select(.internal == false and .exercised == true) | .permission_request | IN(\"fired\", \"not fired\", \"not exercised\")] | all) and (if ([.tools[] | select(.permission_request == \"fired\")] | length > 0) then (.permission_request_payload_keys | length > 0) else (.permission_request_payload_keys | type == \"array\") end)' docs/toolu/evidence/2026-10-19-muse-tool-inventory.json",
    "ac_refs": [
      "AC-2"
    ],
    "depends_on": [
      "S1"
    ],
    "paths": [
      "docs/toolu/evidence/2026-10-19-muse-tool-inventory.json"
    ],
    "input": "live muse exec --approval-mode untrusted sessions with the probe",
    "model": "inherit"
  },
  {
    "id": "S4",
    "title": "Verdict experiments per spec: A0 calibration, ask candidates on PreToolUse and PermissionRequest (hook test + live on-request/untrusted/never), input-rewrite candidates on bash echo original-540, PostToolUse block candidates on edit_file with follow-up records, model quote and git diff",
    "check": "jq -e '(.verdicts.ask.calibration.approval_records | type == \"array\") and (.verdicts.ask.conclusion | type == \"string\" and length > 0) and ([.verdicts.ask.candidates[] | (.event | IN(\"PreToolUse\", \"PermissionRequest\")) and (.hook_test | type == \"string\" and length > 0) and ([.live[\"on-request\", \"untrusted\", \"never\"] | IN(\"prompted\", \"blocked\", \"ran\", \"failed\", \"not-invoked\")] | all)] | all) and (.verdicts.ask.candidates | length >= 8) and (.verdicts.updated_input.conclusion | type == \"string\" and length > 0) and (.verdicts.updated_input.candidates | length >= 3) and ([.verdicts.updated_input.candidates[] | (.hook_test | type == \"string\") and (.tool_result | type == \"string\") and (.ran | IN(\"original-540\", \"rewritten-540\", \"none\"))] | all) and (.verdicts.post_tool_use_block.conclusion | type == \"string\" and length > 0) and (.verdicts.post_tool_use_block.candidates | length >= 2) and ([.verdicts.post_tool_use_block.candidates[] | (.tool_result | type == \"string\") and (.follow_up_records | type == \"array\") and (.model_quote | type == \"string\") and (.persisted | type == \"boolean\")] | all)' docs/toolu/evidence/2026-10-19-muse-tool-inventory.json",
    "ac_refs": [
      "AC-4",
      "AC-5",
      "AC-6"
    ],
    "depends_on": [
      "S1"
    ],
    "paths": [
      "docs/toolu/evidence/2026-10-19-muse-tool-inventory.json"
    ],
    "input": "muse plugins hook test fixtures plus live muse exec sessions driven by control.json rules",
    "model": "inherit"
  },
  {
    "id": "S5",
    "title": "Research doc docs/research/muse-tool-inventory.md: method, toolsets, tool table, examples per family, ignore list, three verdicts, notes for #543/#544/#554; no secrets in doc or evidence",
    "check": "f=docs/research/muse-tool-inventory.md; e=docs/toolu/evidence/2026-10-19-muse-tool-inventory.json; for h in '## Method' '## Toolsets' '## Tool inventory' '## Examples' '## Internal tools' '## Ask verdict' '## Input-rewrite verdict' '## PostToolUse-block verdict' '## Side effects and cleanup' '## Notes for #543, #544 and #554'; do grep -qxF \"$h\" $f || { echo \"missing $h\"; exit 1; }; done; jq -r '.tools | to_entries[] | .key as $t | ($t, (.value.tool_input_keys // [])[] | tostring)' $e | sort -u | while IFS= read -r w; do grep -qF -- \"$w\" $f || { echo \"doc lacks $w\"; exit 1; }; done || exit 1; for t in $(jq -r '.tools | keys[]' $e); do grep -qF \"\\`$t\\`\" $f || { echo \"tool $t not in doc\"; exit 1; }; done; for fam in $(jq -r '[.tools[] | select(.exercised == true and .internal == false) | .family] | unique[]' $e); do grep -qF \"Example \u2014 $fam\" $f || { echo \"doc lacks example for $fam\"; exit 1; }; done; notes=$(awk '/^## Notes for #543, #544 and #554$/{p=1;next} /^## /{p=0} p' $f); for t in $(jq -r '.tools | to_entries[] | select(.value.family | IN(\"delegation\", \"workflow\", \"mcp\")) | .key' $e); do printf '%s' \"$notes\" | grep -qF \"$t\" || { echo \"notes lack $t\"; exit 1; }; done; side=$(awk '/^## Side effects and cleanup$/{p=1;next} /^## /{p=0} p' $f); first=$(jq -r '.cleanup.global_list | split(\"\\n\")[0]' $e); [ -n \"$first\" ] && printf '%s' \"$side\" | grep -qF -- \"$first\" || { echo \"side-effects section lacks the recorded global list\"; exit 1; }; ! grep -nE 'Bearer [A-Za-z0-9]|sk-[A-Za-z0-9]{16}|access_token|refresh_token|\"token\"|/root/' $f $e && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask check-markdown-cli",
    "ac_refs": [
      "AC-1",
      "AC-2",
      "AC-3",
      "AC-4",
      "AC-5",
      "AC-6",
      "AC-7",
      "AC-8",
      "AC-10"
    ],
    "depends_on": [
      "S2",
      "S3",
      "S4",
      "S6"
    ],
    "paths": [
      "docs/research/muse-tool-inventory.md",
      "docs/toolu/evidence/2026-10-19-muse-tool-inventory.json",
      "docs/cli/commands.json",
      "tooling/conventions/markdown-cli.json"
    ],
    "input": "the committed evidence JSON",
    "model": "inherit"
  },
  {
    "id": "S6",
    "title": "Cleanup (runs even when an earlier step is red): XDG_DATA_HOME=<scratch>/xdg-data muse plugins remove toolu-probe-540 --delete-data; record the sandbox and global muse plugins list output in evidence.cleanup; take the after snapshot and diff it (snap.py diff); remove each probe-caused path (cron_delete/edit_memory where the tool can, else delete) and record evidence.side_effects; reject + remove again if trust keys survive",
    "check": "export MUSE_UPDATE_INTERVAL_SECONDS=99999999; out=$(muse plugins list) || exit 1; [ -n \"$out\" ] || exit 1; case \"$out\" in *toolu-probe-540*) echo 'probe still listed'; exit 1;; esac; mk=$(muse plugins marketplace list) || exit 1; case \"$mk\" in *toolu-probe-540*) echo 'probe marketplace listed'; exit 1;; esac; jq -e '(.runtime_capabilities // {}) | keys | map(select(startswith(\"plugin:toolu-probe-540\"))) | length == 0' \"$HOME/.config/muse/settings.json\" && jq -e '(.cleanup.sandbox_list == \"no plugins\") and (.cleanup.global_list | type == \"string\" and (contains(\"toolu-probe-540\") | not)) and (.side_effects | type == \"array\") and ([.side_effects[] | (.removed == true) or (.reason | type == \"string\" and length > 0)] | all)' docs/toolu/evidence/2026-10-19-muse-tool-inventory.json && for p in $(jq -r '.side_effects[] | select(.removed == true) | .path' docs/toolu/evidence/2026-10-19-muse-tool-inventory.json); do q=$(printf '%s' \"$p\" | sed \"s|^~|$HOME|\"); [ ! -e \"$q\" ] || { echo \"still present: $p\"; exit 1; }; done",
    "ac_refs": [
      "AC-8",
      "AC-10"
    ],
    "depends_on": [
      "S1"
    ],
    "input": "live muse state after muse plugins remove toolu-probe-540 --delete-data",
    "model": "inherit",
    "paths": [
      "docs/toolu/evidence/2026-10-19-muse-tool-inventory.json"
    ]
  },
  {
    "id": "S7",
    "title": "Post the findings on #540: one comment (or numbered parts '(n/m)' when over 60,000 characters) with every evidence tool and its tool_input keys, one redacted example per exercised family, the ignore list, the three verdicts and notes for #554, linking the research doc",
    "check": "body=$(gh api repos/Falconiere/toolu/issues/540/comments --paginate --jq '.[].body | select(contains(\"Muse tool inventory\"))') && [ -n \"$body\" ] && for h in 'Internal tools' 'Ask verdict' 'Input-rewrite verdict' 'PostToolUse-block verdict' 'Notes for #543, #544 and #554'; do printf '%s' \"$body\" | grep -qF \"$h\" || { echo \"comment lacks $h\"; exit 1; }; done && jq -r '.tools | to_entries[] | .key as $t | ($t, (.value.tool_input_keys // [])[] | tostring)' docs/toolu/evidence/2026-10-19-muse-tool-inventory.json | sort -u | while IFS= read -r w; do printf '%s' \"$body\" | grep -qF -- \"$w\" || { echo \"comment lacks $w\"; exit 1; }; done && for f in $(jq -r '[.tools[] | select(.exercised == true and .internal == false) | .family] | unique[]' docs/toolu/evidence/2026-10-19-muse-tool-inventory.json); do printf '%s' \"$body\" | grep -qF \"Example \u2014 $f\" || { echo \"comment lacks example for $f\"; exit 1; }; done",
    "ac_refs": [
      "AC-9"
    ],
    "depends_on": [
      "S5",
      "S6"
    ],
    "input": "the research doc's findings",
    "model": "inherit",
    "paths": [
      "docs/toolu/evidence/2026-10-19-muse-tool-inventory.json"
    ]
  },
  {
    "id": "S8",
    "title": "Scope and gates on the final tree: the four files are tracked (git add -f under the ignored docs/toolu) and are the whole diff against origin/main; docs CI checks; full cargo xtask gate",
    "check": "git ls-files --error-unmatch docs/research/muse-tool-inventory.md docs/toolu/evidence/2026-10-19-muse-tool-inventory.json docs/toolu/plans/2026-10-19-muse-tool-inventory-spike.md docs/toolu/specs/2026-10-19-muse-tool-inventory-spike-design.md >/dev/null && [ \"$(git diff --name-only origin/main...HEAD | sort | tr '\\n' ' ')\" = \"$(printf '%s\\n' docs/research/muse-tool-inventory.md docs/toolu/evidence/2026-10-19-muse-tool-inventory.json docs/toolu/plans/2026-10-19-muse-tool-inventory-spike.md docs/toolu/specs/2026-10-19-muse-tool-inventory-spike-design.md | sort | tr '\\n' ' ')\" ] || { echo 'diff scope differs from the four planned files'; git diff --name-only origin/main...HEAD; exit 1; }; export PATH=\"$HOME/.cargo/bin:$PATH\"; mkdir -p \"$HOME/.cache/toolu-540-tmp\" && chmod 700 \"$HOME/.cache/toolu-540-tmp\" && export TMPDIR=\"$HOME/.cache/toolu-540-tmp\"; cargo xtask gate --only context-budget --only ci-paths && cargo xtask check-markdown-cli && cargo xtask gate",
    "ac_refs": [
      "AC-9"
    ],
    "depends_on": [
      "S5",
      "S6",
      "S7"
    ],
    "input": "the whole branch tree (no paths: freshness follows the full diff)",
    "model": "inherit"
  }
]
```

## Critical files

- `docs/research/muse-tool-inventory.md` (new)
- `docs/toolu/evidence/2026-10-19-muse-tool-inventory.json` (new, force-added like the other `docs/toolu` records)
- `docs/toolu/specs/2026-10-19-muse-tool-inventory-spike-design.md`, `docs/toolu/plans/2026-10-19-muse-tool-inventory-spike.md`

## Verification

The tools are exercised end to end by live `muse exec` sessions, with real model calls, real hooks and real tool results. There is no fixture and no mock.

Before writing the evidence JSON, the scratchpad `extract.py` cross-checks every `assistant_tool_calls_committed` call against a hook-log entry. A call with no hook entry is reported as a finding, not dropped.

The boundary cases:

- a family the model will not exercise is marked `exercised: false` with its reason;
- a tool that fails records its `PostToolUseFailure` payload;
- a verdict candidate that Muse classifies `failed` is recorded verbatim.

S6 runs even if an earlier step fails. No user-facing documentation changes: Muse is not a shipped host until #542 onward, and #551 owns those docs.

**Delivery.** Each step's output is committed as soon as its check is green, with a `docs(research):` subject. Files under the ignored `docs/toolu/` need `git add -f`. Per the worker brief, every such commit is pushed to `feat/540-spike-muse-tool-inventory-ask` so a session loss loses nothing. The push-review gate is advisory in this repository (no `gates.pushReview` mode is configured). The commit scope is only:

- the research doc;
- the evidence JSON;
- this plan and its spec.

S8 enforces that scope with `git diff --name-only origin/main...HEAD`. Then:

1. `toolu ledger run docs/toolu/plans/2026-10-19-muse-tool-inventory-spike.md --verify` re-runs every step against the final committed tree.
2. `toolu-review:review` records version 2 review state.
3. `toolu ledger verdict status` must report `overall: ready`.
4. The final push.
5. The PR targets `main`, with a `docs(research):` conventional title. Its body opens with `Closes Falconiere/toolu#540` and `Part of Falconiere/toolu#539`, then gives the summary and the verification evidence.
6. `toolu epic report pr-open --pr <n> --status-file /root/.claude/epics/falconiere-toolu-539/status/falconiere-toolu-540.json`, then `toolu epic report babysit --pr <n> --status-file …` (same file), and the worker goes idle. The engine babysits.

Delivery needs an authenticated `gh api user` and a non-default branch.

**Failure runbook.** When any of S1–S5 goes red, or a run hangs, the S6 cleanup runs by hand before anything else. This clears the probe and its trust keys from the shared Muse state that the #541 worker also uses. Then the failing step is repaired and re-run with the probe re-installed.
