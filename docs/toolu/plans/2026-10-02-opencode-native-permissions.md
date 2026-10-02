# OpenCode native permissions and gate advisories (OP-05) — Plan

**Date:** 2026-10-02   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-02-opencode-native-permissions-design.md   **Topic:** Preserve native permission decisions and deliver pre-tool advice on pinned OpenCode.

## Evidence and approach

Recalled `6990a41a` and inspected `tools/toolu-opencode/contract/probe-results.json`: on OpenCode 1.18.34 a before-hook throw stops execution; the host does not invoke `permission.ask` in the tested native ask path; an after-hook output append reaches the model. Inspected `createToolBeforeHandler`, `createGateDecider`, `gateMode`, `supportsAsk`, `encodeDecision`, and the existing real-host scripted-provider harness. The adapter already preserves native permission checks by returning normally on allow. Correct the advertised capability, retain native enforcement, and transport advice through a per-call before/after pair. Test real rejection and side effects before documenting the behavior.

## Workstream summary

Capability translation → per-call advice lifecycle → pinned-host composition proof → docs and full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "capability",
    "title": "Remove the false OpenCode ask/allow effect claim and preserve stricter effects in the compatibility helper; verify guardrail and judgement fallback without changing other hosts",
    "ac_refs": ["AC-1", "AC-2", "AC-4"],
    "paths": [
      "packages/toolu-core/src/host/**",
      "packages/toolu-core/src/config/**",
      "plugins/*/hooks/dist/**",
      "tools/toolu-conformance/src/harness/**",
      "tools/toolu-opencode/src/adapter/permission-map.ts",
      "tools/toolu-opencode/src/adapter/__tests__/permission-map.test.ts"
    ],
    "input": "Existing native deny and ask effects, toolu allow/advisory/deny/ask decisions, and real temp-project gate config with protectedFiles and judgement modes set to ask",
    "check": "bun test --timeout 60000 packages/toolu-core/src/host/__tests__ packages/toolu-core/src/config/__tests__/gate-mode.test.ts tools/toolu-opencode/src/adapter/__tests__/permission-map.test.ts tools/toolu-conformance/src/harness/__tests__/host-encode.test.ts tools/toolu-conformance/src/harness/__tests__/hosts.test.ts && bun run check:plugin-bundles",
    "model": "inherit"
  },
  {
    "id": "advice",
    "title": "Add per-instance before/after advice delivery keyed by session and call ID; assert deny over advice, model-visible output, bounded pending state, expiry, retries, disposal and preserved original output",
    "ac_refs": ["AC-2", "AC-3"],
    "depends_on": ["capability"],
    "paths": [
      "tools/toolu-opencode/src/adapter/**",
      "tools/toolu-opencode/src/plugin/**",
      "packages/toolu-core/src/dispatch/**",
      "plugins/toolu/hooks/src/pre-tools/**",
      "plugins/ast-grep/hooks/dist/search-nudge.js"
    ],
    "input": "Real temp git project, committed ast-grep registry bundle producing advisory, protected-file deny, two sessions with identical call IDs, a reused ID, and malformed after output",
    "check": "bun test --timeout 60000 tools/toolu-opencode/src/adapter/__tests__ tools/toolu-opencode/src/plugin/__tests__ packages/toolu-core/src/dispatch/__tests__",
    "model": "inherit"
  },
  {
    "id": "live",
    "title": "Drive the pinned host with native deny/ask/rejection, gate ask fallback, a provider-visible advisory in the next tool result, other-plugin denials in both orders and repeated calls; check exact markers and tool states",
    "ac_refs": ["AC-1", "AC-2", "AC-3"],
    "depends_on": ["advice"],
    "paths": [
      "tooling/src/opencode-permissions-smoke.ts",
      "tooling/src/opencode-host/**",
      "tools/toolu-opencode/src/**",
      "tools/toolu-opencode/contract/**",
      "plugins/toolu/**",
      "plugins/ast-grep/**",
      "package.json"
    ],
    "input": "Pinned opencode-ai@1.18.34 in isolated project/profile; scripted loopback provider; real bash/write calls and marker files; native permission deny/ask with two rejected attempts, protectedFiles ask, selected registry advisory and combined deny, second plugin before/after toolu; inspect provider tool-result content and confirm denied marker bytes remain absent",
    "check": "bun run smoke:opencode-permissions && bun run smoke:opencode-pretool && bun run smoke:opencode-entry",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "Document the verified native permission lifecycle, ask fallback and advisory channel; synchronize generated resource mirrors",
    "ac_refs": ["AC-4"],
    "depends_on": ["live"],
    "paths": ["**"],
    "input": "Observed pinned-host smoke results and approved spec",
    "check": "bun run check:opencode-surface && bun run test:portable-core",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Run the full repository test, package and conformance gate on the final branch",
    "ac_refs": ["AC-5"],
    "depends_on": ["docs"],
    "paths": ["**"],
    "input": "Whole branch with implementation, pinned-host results and synchronized documentation",
    "check": "bun run test",
    "model": "inherit"
  }
]
```

## Critical files

Modify `packages/toolu-core/src/host/{host-encode,host-events}.ts`, `tools/toolu-opencode/src/adapter/{permission-map,tool-before}.ts`, and `tools/toolu-opencode/src/plugin/{hooks,enforcement}.ts`. Add a small per-instance advice helper only if it keeps the adapter focused. Extend colocated host, adapter, plugin and conformance tests. Rebuild committed hook bundles that inline the changed shared host code. Add `tooling/src/opencode-permissions-smoke.ts` and focused scenarios under `tooling/src/opencode-host/`; reuse its provider, session and pinned CLI helpers. Update root `package.json`, `docs/opencode.md`, `docs/portable-core.md`, the host contract and generated resource mirrors.

## Deviations

The full gate found that 34 existing hook bundles inline the shared host code, so they must be rebuilt and drift checked. The conformance harness also still asserted the superseded mutable-effect encoder, so its callback contract tests were added to the capability step. The direct full run additionally exposed a duplicate smoke runner and an unused re-export; those were fixed in the smoke helper before repeating the live step.

## Verification

The host itself must reject native deny/ask and a user rejection without target side effects. A toolu guardrail ask blocks, while a judgement ask and registry advisory appear in the next model-visible tool result only for the matching successful call. A combined deny/advisory and either load order of a second denying plugin stop execution. Direct tests cover session/call isolation, retry, expiry and bounded storage. Existing hosts retain their encoder results. Re-run affected checks after any rebase, then run `bun run test`, final ledger verification, review and delivery preflight before pushing the PR.

## Plan review

- Live step: 🟡 should-fix (resolved): the draft named rejection and advice but did not require two rejected attempts or inspection of provider tool-result content. Both are now explicit inputs and observations.
- Advice/docs paths: 🟡 should-fix (resolved): the advice check reads the ast-grep bundle, and surface regeneration reads broad inputs. The bundle is declared and the docs step uses a conservative full-tree freshness scope.
- Alignment: Jev rated the initial plan near the middle level; the listed concrete observations and path correction address the identified gaps. Every AC is referenced by a runnable step, and the final step is the full gate.
- **Status:** Approved.
