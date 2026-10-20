# Muse live end-to-end verification — Plan

**Date:** 2026-10-20   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-20-muse-live-e2e-design.md   **Topic:** the #553 acceptance run of toolu in real Muse sessions, recorded as redacted evidence

## Evidence and approach

The approved spec decides the run. Its shape reuses:
- the probe shapes and project layout of `crates/xtask/src/host_smoke_project.rs`
  and `host_smoke_steps.rs` (`.env` = `KEEP=1`, `sample.ts`, the `LIMITS` config,
  `feat/push-smoke` with a bare origin);
- the seeded-checkout practice of `docs/toolu/evidence/2026-10-20-muse-installer.md`
  (#549);
- the trust semantics in memories 0817ef1f and f6a5ddca.

The `/usr/local/bin` mask comes from the `verify-root-host-gates` project skill.
`muse exec` was confirmed to run in an isolated profile with only `auth.json`
copied. The sessions spend credits and are not deterministic, so they run by
hand. The ledger then checks the committed evidence file, as #549's S6 did.

## Workstream summary

Clean-machine setup and trust (check 8 then 1) → quality and commit (2, 3) →
push (4) → internal tools and fail-closed (5, 6) → doctor (7) → evidence redaction
→ docs gates → full gate. A deviation stops the sequence at its step: the bug is
filed against the owning sub-issue, and the row records FAIL with the link (spec
AC-10).

## Steps (machine-readable)

```json
[
  {
    "id": "S1",
    "title": "Clean-machine setup; install toolu without --approve and show a .env write proceeds, approve and show it blocked (check 8); .env write blocked under block config and default preset (check 1)",
    "check": "grep -Eq '^\\| 8 \\|.*\\| PASS \\|' docs/toolu/evidence/2026-10-20-muse-live-e2e.md && grep -q '^## Check 8' docs/toolu/evidence/2026-10-20-muse-live-e2e.md && grep -Eq '^\\| 1 \\|.*\\| PASS \\|' docs/toolu/evidence/2026-10-20-muse-live-e2e.md && grep -q '^## Check 1' docs/toolu/evidence/2026-10-20-muse-live-e2e.md && jq -e '.checks[\"8\"] | (.facts | length > 0 and all(.[]; . == true)) and (.sessions | length > 0) and all(.sessions[]; .hook_failures == 0)' docs/toolu/evidence/2026-10-20-muse-live-e2e.json >/dev/null && jq -e '.checks[\"1\"] | (.facts | length > 0 and all(.[]; . == true)) and (.sessions | length > 0) and all(.sessions[]; .hook_failures == 0)' docs/toolu/evidence/2026-10-20-muse-live-e2e.json >/dev/null",
    "ac_refs": [
      "AC-8",
      "AC-1"
    ],
    "paths": [
      "docs/toolu/evidence/2026-10-20-muse-live-e2e.md",
      "docs/toolu/evidence/2026-10-20-muse-live-e2e.json"
    ],
    "input": "real Muse 1.4.11 muse exec sessions in an env -i scratch home (XDG_* isolated, /usr/local/bin masked by unshare -m) with target/debug/toolu built from origin/main 04f29e95 installed as $HOME/.local/bin/toolu and plugins installed by toolu plugins install --host muse from a checkout seeded with a local v8.0.4 tag",
    "model": "inherit"
  },
  {
    "id": "S2",
    "title": "Install every plugin --approve; session edit breaks maxFileLines, gate file failing and toolu status shows it (check 2); commit blocked with HEAD unchanged, then fixed and committed (check 3)",
    "check": "grep -Eq '^\\| 2 \\|.*\\| PASS \\|' docs/toolu/evidence/2026-10-20-muse-live-e2e.md && grep -q '^## Check 2' docs/toolu/evidence/2026-10-20-muse-live-e2e.md && grep -Eq '^\\| 3 \\|.*\\| PASS \\|' docs/toolu/evidence/2026-10-20-muse-live-e2e.md && grep -q '^## Check 3' docs/toolu/evidence/2026-10-20-muse-live-e2e.md && jq -e '.checks[\"2\"] | (.facts | length > 0 and all(.[]; . == true)) and (.sessions | length > 0) and all(.sessions[]; .hook_failures == 0)' docs/toolu/evidence/2026-10-20-muse-live-e2e.json >/dev/null && jq -e '.checks[\"3\"] | (.facts | length > 0 and all(.[]; . == true)) and (.sessions | length > 0) and all(.sessions[]; .hook_failures == 0)' docs/toolu/evidence/2026-10-20-muse-live-e2e.json >/dev/null",
    "ac_refs": [
      "AC-2",
      "AC-3"
    ],
    "depends_on": [
      "S1"
    ],
    "paths": [
      "docs/toolu/evidence/2026-10-20-muse-live-e2e.md",
      "docs/toolu/evidence/2026-10-20-muse-live-e2e.json"
    ],
    "input": "real Muse 1.4.11 muse exec sessions in an env -i scratch home (XDG_* isolated, /usr/local/bin masked by unshare -m) with target/debug/toolu built from origin/main 04f29e95 installed as $HOME/.local/bin/toolu and plugins installed by toolu plugins install --host muse from a checkout seeded with a local v8.0.4 tag",
    "model": "inherit"
  },
  {
    "id": "S3",
    "title": "Push to a bare origin blocked by push review with no remote ref, ask mode also blocks, then toolu review write-state --findings-count 0 and the push lands (check 4)",
    "check": "grep -Eq '^\\| 4 \\|.*\\| PASS \\|' docs/toolu/evidence/2026-10-20-muse-live-e2e.md && grep -q '^## Check 4' docs/toolu/evidence/2026-10-20-muse-live-e2e.md && jq -e '.checks[\"4\"] | (.facts | length > 0 and all(.[]; . == true)) and (.sessions | length > 0) and all(.sessions[]; .hook_failures == 0)' docs/toolu/evidence/2026-10-20-muse-live-e2e.json >/dev/null",
    "ac_refs": [
      "AC-4"
    ],
    "depends_on": [
      "S2"
    ],
    "paths": [
      "docs/toolu/evidence/2026-10-20-muse-live-e2e.md",
      "docs/toolu/evidence/2026-10-20-muse-live-e2e.json"
    ],
    "input": "real Muse 1.4.11 muse exec sessions in an env -i scratch home (XDG_* isolated, /usr/local/bin masked by unshare -m) with target/debug/toolu built from origin/main 04f29e95 installed as $HOME/.local/bin/toolu and plugins installed by toolu plugins install --host muse from a checkout seeded with a local v8.0.4 tag",
    "model": "inherit"
  },
  {
    "id": "S4",
    "title": "No-tool prompt and routine read/edit/bash session with internal tools: no failed hook events (check 5); binary renamed mid-run blocks bash with the install advisory, restore re-gates (check 6)",
    "check": "grep -Eq '^\\| 5 \\|.*\\| PASS \\|' docs/toolu/evidence/2026-10-20-muse-live-e2e.md && grep -q '^## Check 5' docs/toolu/evidence/2026-10-20-muse-live-e2e.md && grep -Eq '^\\| 6 \\|.*\\| PASS \\|' docs/toolu/evidence/2026-10-20-muse-live-e2e.md && grep -q '^## Check 6' docs/toolu/evidence/2026-10-20-muse-live-e2e.md && jq -e '.checks[\"5\"] | (.facts | length > 0 and all(.[]; . == true)) and (.sessions | length > 0) and all(.sessions[]; .hook_failures == 0)' docs/toolu/evidence/2026-10-20-muse-live-e2e.json >/dev/null && jq -e '.checks[\"6\"] | (.facts | length > 0 and all(.[]; . == true)) and (.sessions | length > 0) and all(.sessions[]; .hook_failures == 0)' docs/toolu/evidence/2026-10-20-muse-live-e2e.json >/dev/null",
    "ac_refs": [
      "AC-5",
      "AC-6"
    ],
    "depends_on": [
      "S3"
    ],
    "paths": [
      "docs/toolu/evidence/2026-10-20-muse-live-e2e.md",
      "docs/toolu/evidence/2026-10-20-muse-live-e2e.json"
    ],
    "input": "real Muse 1.4.11 muse exec sessions in an env -i scratch home (XDG_* isolated, /usr/local/bin masked by unshare -m) with target/debug/toolu built from origin/main 04f29e95 installed as $HOME/.local/bin/toolu and plugins installed by toolu plugins install --host muse from a checkout seeded with a local v8.0.4 tag",
    "model": "inherit"
  },
  {
    "id": "S5",
    "title": "toolu doctor in the project: no failing line, Muse roots, every plugin approved (check 7)",
    "check": "grep -Eq '^\\| 7 \\|.*\\| PASS \\|' docs/toolu/evidence/2026-10-20-muse-live-e2e.md && grep -q '^## Check 7' docs/toolu/evidence/2026-10-20-muse-live-e2e.md && jq -e '.checks[\"7\"] | (.facts | length > 0 and all(.[]; . == true)) and (.sessions | length > 0) and all(.sessions[]; .hook_failures == 0)' docs/toolu/evidence/2026-10-20-muse-live-e2e.json >/dev/null",
    "ac_refs": [
      "AC-7"
    ],
    "depends_on": [
      "S4"
    ],
    "paths": [
      "docs/toolu/evidence/2026-10-20-muse-live-e2e.md",
      "docs/toolu/evidence/2026-10-20-muse-live-e2e.json"
    ],
    "input": "real Muse 1.4.11 muse exec sessions in an env -i scratch home (XDG_* isolated, /usr/local/bin masked by unshare -m) with target/debug/toolu built from origin/main 04f29e95 installed as $HOME/.local/bin/toolu and plugins installed by toolu plugins install --host muse from a checkout seeded with a local v8.0.4 tag",
    "model": "inherit"
  },
  {
    "id": "S6",
    "title": "Evidence file complete and redacted: eight PASS rows (or a linked issue per FAIL), contract-verified statement, no scratch path, session id or credential",
    "check": "test \"$(grep -cE '^\\| [1-8] \\|.*\\| PASS \\|' docs/toolu/evidence/2026-10-20-muse-live-e2e.md)\" = 8 && grep -q 'enforcement contract' docs/toolu/evidence/2026-10-20-muse-live-e2e.md && ! grep -Eq '/tmp/claude-0|\\.cache/toolu-553|[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}|sk-[A-Za-z0-9]|api[_-]?key' docs/toolu/evidence/2026-10-20-muse-live-e2e.md && ! grep -Eq '/tmp/claude-0|\\.cache/toolu-553|[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}' docs/toolu/evidence/2026-10-20-muse-live-e2e.json && jq -e '.checks|keys == [\"1\",\"2\",\"3\",\"4\",\"5\",\"6\",\"7\",\"8\"]' docs/toolu/evidence/2026-10-20-muse-live-e2e.json >/dev/null",
    "ac_refs": [
      "AC-9",
      "AC-10"
    ],
    "depends_on": [
      "S5"
    ],
    "paths": [
      "docs/toolu/evidence/2026-10-20-muse-live-e2e.md",
      "docs/toolu/evidence/2026-10-20-muse-live-e2e.json"
    ],
    "input": "the committed evidence file",
    "model": "inherit"
  },
  {
    "id": "S7",
    "title": "Markdown command drift and context budget on the new docs",
    "check": "cargo xtask check-markdown-cli && cargo xtask gate --only context-budget",
    "ac_refs": [
      "AC-9"
    ],
    "depends_on": [
      "S6"
    ],
    "paths": [
      "docs",
      "AGENTS.md",
      "tooling/conventions/markdown-cli.json"
    ],
    "input": "docs/toolu/evidence/2026-10-20-muse-live-e2e.md and the generated docs/cli/commands.json",
    "model": "inherit"
  },
  {
    "id": "S8",
    "title": "Full Rust quality gate on the final tree",
    "check": "mkdir -p \"$TMPDIR/empty-local-bin\" && env -u HERDR_SOCKET_PATH -u HERDR_ENV -u HERDR_PANE_ID -u HERDR_TAB_ID -u HERDR_WORKSPACE_ID -u HERDR_BIN_PATH -u HERDR_CONFIG_PATH unshare -m -- sh -c 'mount --make-rprivate / && mount --bind \"$TMPDIR/empty-local-bin\" /usr/local/bin && capsh --drop=cap_dac_override,cap_dac_read_search,cap_sys_ptrace -- -c \"RUST_TEST_THREADS=1 TOOLU_RESOURCE_HOME=$TMPDIR/isolated-resources cargo xtask gate\"'",
    "ac_refs": [
      "AC-10"
    ],
    "depends_on": [
      "S7"
    ],
    "paths": [
      "crates",
      "fixtures",
      "docs",
      "plugins",
      "AGENTS.md"
    ],
    "input": "the branch",
    "model": "inherit"
  }
]
```

## Critical files

- `docs/toolu/evidence/2026-10-20-muse-live-e2e.json` (new): one entry per check, holding
  `sessions[]` (the prompt, the tool events extracted from the raw `muse exec --json`
  records — `tool_name`, `outcome` and the first 400 characters of the result text — and
  `hook_failures`, the count of hook-failure records) and `facts`, named booleans read
  from disk or the transcript (for example `env_unchanged` and `head_unchanged_after_block`)
  that must all be true. It is redacted like the Markdown.
- `docs/toolu/evidence/2026-10-20-muse-live-e2e.md` (new; force-added, since `docs/toolu/` is
  ignored and its tracked files are force-added).
- `docs/toolu/specs/2026-10-20-muse-live-e2e-design.md`, this plan.
- No source file is expected. A wiring-only fix, if one is needed, adds its own
  step with a colocated test.

## Verification

End-to-end, every check runs against real Muse 1.4.11 and the real built
binary, and each verdict is backed by on-disk state:
- `.env` bytes;
- `git rev-parse HEAD` before and after;
- `git for-each-ref` on the bare remote;
- the gate file and `toolu status`.

Each check also covers its boundary:
- an unapproved plugin (inert);
- the default preset (ask → block);
- ask-mode push (block, no waiver on Muse);
- a renamed binary, then restored.

Delivery runs `toolu ledger run <plan> --verify`, `toolu-review:review` and
`toolu ledger verdict status` before the push. A comment on #553 links the
evidence.

## Plan review

Round 1 (Jev: alignment `noul` 0.51; most material gap "the checks verify only an evidence file", 0.98):

- S1–S6: 🔴 blocker: the checks only grepped my own verdict table, so a hand-typed PASS would pass. Fixed: each check now also needs the machine-readable `…-e2e.json`. Its tool events are extracted from the raw `muse exec --json` records, its `facts` are read from disk, and every fact must be true with zero hook failures.
- Approach: 🟡 should-fix (accepted limitation): live sessions cannot rerun inside the ledger, because they spend credits and the model is not deterministic. The spec records this, and #552's `host-smoke --host muse` is the repeatable form.
- S6: 🔵 consider: the redaction check now covers the JSON file too.

Round 2: every AC (1–10) maps to a step, checks are runnable, paths cover what each check reads, the order is linear, and delivery runs `toolu ledger run --verify`, the review and the verdict before the push. **Status:** Approved.
