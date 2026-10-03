# OpenCode ast-grep search enforcement and savings — Plan

**Date:** 2026-10-03 **Status:** Approved **Spec:** docs/toolu/specs/2026-10-03-opencode-ast-grep-search-design.md **Topic:** OP-13 (#347): ast-grep nudge, byte-savings report and skill on the pinned OpenCode host

## Evidence and approach

- A throwaway run of the real bundles through `createToolBeforeHandler` and `createToolPostHandler` showed the nudge paths already work. `grep "function foo"` and `bash rg 'fn main' src` got STOP, `bash rg TODO src` got the generic text, and `grep TODO` and `ls | grep x` got none. byte-savings recorded 46 bytes for a `line1\nline2` result, which is the size of the metadata JSON. Cause: `tool-post.ts` sends the text only as `tool_output`, while `byte-savings.ts` measures `tool_response`.
- The pinned host runs on this machine: a baseline `bun run smoke:opencode-posttool posttool.shell` passed 1/1 on opencode-ai@1.18.34.
- Reuse: `pretool-shared.ts` (`session`, `runPretoolScenarios`), `prepareSdk` from `scenarios-posttool-smoke.ts` (to be exported), `host-run.ts` (`runHost`, `toolStates`), `scenario.ts` (`finalMessages`), `tooluProcessEnv` for the hermetic `register.js` run. The generated skill id is `ast-grep-ast-grep`. ast-grep's golden corpus runs only the `claude` host (`hostsOf`), so a report gated on host `opencode` cannot move it.
- Decisions (spec): the adapter adds `tool_response.output`. byte-savings appends the session report after ast-grep runs on OpenCode only (Jev `visible` 0.99, `astgrep` 0.64). The report function moves to a shared lib.

## Workstream summary

adapter measurement fix → shared report lib and OpenCode-only report → skill fix → hermetic real-bundle tests and rebuilt bundles → pinned-host smoke → docs and mirrors → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "adapter",
    "title": "Carry the host's result text in tool_response.output so post modules measure the real result; pin it with a real post module that reads tool_response.output",
    "ac_refs": [
      "AC-2"
    ],
    "paths": [
      "tools/toolu-opencode/src/adapter/**"
    ],
    "input": "Real temp git projects; a fixture post-tools.d module on disk recording event.toolOutput; shell exit 0 and 3, interrupted call (no dispatch), non-string output (no dispatch), read result text",
    "check": "bun test --timeout 60000 tools/toolu-opencode/src/adapter/__tests__/tool-post-response.test.ts tools/toolu-opencode/src/adapter/__tests__/tool-post.test.ts tools/toolu-opencode/src/adapter/__tests__/tool-advice.test.ts",
    "model": "inherit"
  },
  {
    "id": "module",
    "title": "Move report() and ledger parsing to lib/savings-report.ts; byte-savings returns the session report after an ast-grep run on OpenCode only (header comment updated); fix the skill's dead detect.sh pointer; rebuild bundles; regenerate the skill mirror",
    "ac_refs": [
      "AC-3",
      "AC-6"
    ],
    "depends_on": [
      "adapter"
    ],
    "paths": [
      "plugins/ast-grep/**",
      "packages/toolu-core/src/**",
      "tools/toolu-conformance/src/harness/**",
      "tools/toolu-opencode/generated/**",
      "tools/toolu-opencode/scripts/**"
    ],
    "input": "Golden byte-savings and report corpora (host claude); real register.js-published byte-savings bundle run through the core post dispatcher with host opencode vs claude on a real temp ledger; corrupt ledger line; non-ast-grep kinds",
    "check": "bun test --timeout 60000 plugins/ast-grep/hooks/src/__tests__/ && bun run tooling/src/build-plugins.ts --check && bun run generate:opencode-surface && bun run check:opencode-surface && bun run typecheck",
    "model": "inherit"
  },
  {
    "id": "hermetic",
    "title": "Real register.js-published modules driven by the OpenCode before/after handlers: nudge routes, ledger bytes, report visibility and boundaries",
    "ac_refs": [
      "AC-1",
      "AC-2",
      "AC-3"
    ],
    "depends_on": [
      "module"
    ],
    "paths": [
      "tools/toolu-opencode/src/adapter/**",
      "tools/toolu-opencode/src/host/**",
      "plugins/ast-grep/hooks/**",
      "packages/toolu-core/src/**",
      "tools/toolu-conformance/src/harness/**"
    ],
    "input": "Temp git project with src/app.ts and notes.md; real register.js run under tooluProcessEnv writes into a temp data root; OpenCode grep/bash/read/glob shapes with host metadata (including truncated); real ast-grep stdout; read/glob results carry no savings text; PATH without ast-grep; skills.ast-grep=false; duplicate, 8 parallel, interrupted, non-string and empty-output after calls",
    "check": "bun test --timeout 60000 tools/toolu-opencode/src/adapter/__tests__/ast-grep-nudge.test.ts tools/toolu-opencode/src/adapter/__tests__/ast-grep-savings.test.ts",
    "model": "inherit"
  },
  {
    "id": "live",
    "title": "Pinned-host smoke (adds package.json script, scenarios file, and exports prepareSdk from scenarios-posttool-smoke.ts): native skill load, the skill's search example on a real project, nudges and report in tool messages, the session ledger",
    "ac_refs": [
      "AC-4",
      "AC-5"
    ],
    "depends_on": [
      "hermetic"
    ],
    "paths": [
      "tooling/src/opencode-ast-grep-smoke.ts",
      "tooling/src/opencode-host/**",
      "tools/toolu-opencode/src/**",
      "tools/toolu-opencode/generated/**",
      "plugins/ast-grep/**",
      "plugins/toolu/**",
      "package.json"
    ],
    "input": "Pinned opencode-ai@1.18.34 (first run needs network to resolve the host; ast-grep on PATH, /opt/homebrew/bin here) in an isolated git project and profile with a scripted loopback provider, toolu and ast-grep enabled, src/app.ts and notes.md; skill, grep, bash and read calls",
    "check": "bun run smoke:opencode-ast-grep",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "Document OpenCode ast-grep behavior, ledger, report and smoke in docs/opencode.md, docs/ast-grep/README.md, docs/registry.md, plugin README and smoke lists; regenerate OpenCode mirrors",
    "ac_refs": [
      "AC-5",
      "AC-6"
    ],
    "depends_on": [
      "live"
    ],
    "paths": [
      "docs/**",
      "plugins/ast-grep/README.md",
      "tools/toolu-opencode/generated/**",
      "tools/toolu-opencode/contract/**",
      "tools/toolu-opencode/scripts/**"
    ],
    "input": "Observed smoke results; docs/opencode.md, docs/opencode-host-contract.md, docs/portable-core.md, plugins/ast-grep/README.md and generated mirrors",
    "check": "bun run generate:opencode-surface && bun run check:opencode-surface && bun run test:portable-core",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Run the complete repository gate on the final branch",
    "ac_refs": [
      "AC-6"
    ],
    "depends_on": [
      "docs"
    ],
    "paths": [
      "**"
    ],
    "input": "Whole branch with the hermetic and pinned-host checks green",
    "check": "bun run test",
    "model": "inherit"
  },
  {
    "id": "final-live",
    "title": "Re-run the pinned-host smoke on the final branch after docs and mirror regeneration; its output goes in the PR body",
    "ac_refs": [
      "AC-4",
      "AC-5"
    ],
    "depends_on": [
      "gate"
    ],
    "paths": [
      "**"
    ],
    "input": "Final branch, pinned opencode-ai@1.18.34, same isolated scenario",
    "check": "bun run smoke:opencode-ast-grep",
    "model": "inherit"
  }
]
```

## Critical files

- `tools/toolu-opencode/src/adapter/tool-post.ts` (modify)
- `plugins/ast-grep/hooks/src/lib/savings-report.ts` (new), `byte-savings.ts`, `byte-savings-report.ts` (modify), `hooks/dist/byte-savings.js`, `hooks/dist/byte-savings-report.js` (rebuilt)
- `plugins/ast-grep/skills/ast-grep/SKILL.md`, `plugins/ast-grep/README.md`
- `tools/toolu-opencode/src/adapter/__tests__/ast-grep-nudge.test.ts`, `ast-grep-savings.test.ts` and their shared `ast-grep-fixture.ts` (new)
- `tooling/src/opencode-ast-grep-smoke.ts`, `tooling/src/opencode-host/scenarios-ast-grep-smoke.ts` (new); `scenarios-posttool-smoke.ts` (export `prepareSdk`)
- `package.json` (`smoke:opencode-ast-grep`)
- `docs/opencode.md`, `docs/opencode-host-contract.md`, `docs/portable-core.md`, `docs/ast-grep/README.md`, `docs/registry.md`, generated mirrors
- New colocated tests: `plugins/ast-grep/hooks/src/__tests__/byte-savings-opencode.test.ts`, `tools/toolu-opencode/src/adapter/__tests__/tool-post-response.test.ts`

## Verification

End to end, a pinned OpenCode session with ast-grep enabled loads the skill, runs its search example and gets the matching function. It sees exactly one nudge on each structural or file search and none on piped, literal or non-code searches. After the ast-grep run it sees the session savings report. The data root holds one accurate ledger line per measured call. Boundaries: missing ast-grep (WARN text), opt-out, duplicate, parallel, interrupted and empty-output after calls, a corrupt ledger, host `claude` unchanged. Docs and mirrors are kept in sync by `check:opencode-surface`, and `bun run test` covers Claude and Codex.

PR readiness: scoped commits; `bun plugins/toolu/hooks/dist/plan-ledger.js run <plan> --verify`; `toolu-review:review`; `verdict.js status` ready; push. The PR body carries the final `smoke:opencode-ast-grep` output, because no CI gate runs it before OP-28 (#362).

## Deviations

- adapter: the `tool_response.output` case lives in its own `tool-post-response.test.ts`; `tool-post.test.ts` is at the 300-line lint limit.
- hermetic: `ast-grep-modules.test.ts` became `ast-grep-nudge.test.ts` and `ast-grep-savings.test.ts` over a shared `ast-grep-fixture.ts`, to stay under the per-file and per-function line limits.
