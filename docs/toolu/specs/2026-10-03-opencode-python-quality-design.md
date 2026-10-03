# OpenCode Python post-edit quality — Design

**Date:** 2026-10-03 **Status:** Approved **Author:** Claude Code **Topic:** OP-19 native python-quality enforcement

## Problem

OpenCode registers the python-quality post-tool module, but registration alone does not prove that native edits run its rules. An invalid Python edit must reach the model and leave a failing quality gate that refuses later commit and push attempts. Nothing in the repository exercises the shipped python-quality bundle through the OpenCode bridge or the pinned host.

## Non-Goals

1. Add a linter or type checker (`ruff`, `pylint`, `mypy`). python-quality is static-only by contract; its toolchain is the `python3` prerequisite plus a real `ast-grep` Python scan for the no-mocks rule.
2. Change the shared OpenCode post-tool bridge, the core dispatcher or the python-quality rules without a reproduced defect.
3. Change python-quality's linked-worktree behavior. Unlike ts-quality, it checks linked worktrees on Claude Code and Codex, and OpenCode keeps that.
4. Treat a post-tool diagnostic as undoing a completed edit.

## Architecture

Reuse the OP-06 `tool.execute.after` bridge and the OP-18 (#352) dispatcher option `continuePostBlocks`, which the OpenCode bridge already passes for every module. The python-quality SessionStart bundle publishes `python-quality@toolu__python-quality.js`. Its existing module requires a Python marker (`pyproject.toml`, `setup.py`, `setup.cfg` or `requirements.txt`) at the git toplevel and `python3` on `PATH`, then calls `fileQuality` on `.py` paths with `skipLinkedWorktrees: false`.

A probe test with the committed bundle in a real git project already passes for writes, edits, multi-file patches with move and delete, selection, prerequisites, project isolation and the linked worktree. No production change is needed. This issue adds the missing evidence:

- `tools/toolu-opencode/src/adapter/__tests__/python-quality-post.test.ts`, a real-bundle adapter test that runs real `git`, `python3` detection and `ast-grep` in sandboxed git projects.
- `tooling/src/opencode-host/scenarios-python-quality-smoke.ts` plus `tooling/src/opencode-python-quality-smoke.ts` and a `smoke:opencode-python-quality` script, mirroring the ts-quality pinned-host smoke.
- Documentation of the tested OpenCode behavior and prerequisites.

The decisive trade-off is evidence over new code: the shared bridge is already generic, so a python-specific code path would add surface without a demonstrated defect. Jev agreed with keeping the existing linked-worktree behavior (choice A, 0.95) and with reading "real language toolchain" as `python3` plus the ast-grep Python scan (0.88).

## Interfaces / Schema

- Selection: the existing `.opencode/toolu/plugins.json` `enabled` list (`["toolu", "python-quality"]`) and `selectedPluginSpecs` passed to `dispatchPostTool`. No new configuration.
- Native tools: OpenCode's pinned `tool.execute.after` receives `write`, `edit` and `apply_patch` completed args and a mutable text result; the bridge appends a bounded `QUALITY VIOLATION` diagnostic.
- Gate: `.opencode/tmp/quality-gate-status.json` under the project's git toplevel. A file entry keyed by the edited path as given (absolute or relative), `source: "python-quality-hook"`, `status: "failing"` on a violation; a clean edit, deletion or move source clears it.
- Script: `bun run smoke:opencode-python-quality [scenario-id]` exits 0 when every scenario passes on the pinned OpenCode 1.18.34 CLI.

## Failure modes and edge cases

- python-quality not selected, no Python marker at the toplevel, or no `python3` on `PATH`: no python-quality diagnostic and no gate file. The bridge may still run other selected modules.
- A non-`.py` file (`notes.md` containing `except: pass`): not checked, no entry.
- Multi-file patch: each added or updated `.py` destination is checked once. A deletion and the source side of a move clear their prior entries; a moved destination is checked. Two violating destinations both stay failing and both appear in the appended diagnostic.
- `ast-grep` absent: the no-mocks rule is skipped, as on other hosts; a broken scan is itself a violation. The tests assert `ast-grep` is installed so a missing tool cannot make them pass vacuously.
- Linked worktree: checks run, and the gate file belongs to the linked worktree's toplevel, not the main checkout.
- Two independent projects keep separate gate files; clearing one does not clear the other.
- A thrown host tool error has no after callback; an interrupted bash result cannot clear failure state (OP-06 behavior, unchanged).

## Acceptance criteria

- **AC-1:** With python-quality selected in a real Python project, native OpenCode write and edit calls run the shipped rules on `.py` files; a one-line `except ...: pass` or bare `except:` produces a visible diagnostic and a failing per-file entry with `source: "python-quality-hook"`, and a clean edit returns the gate to passing.
- **AC-2:** A native multi-file patch that moves a failing `.py` file to a still-invalid destination, deletes another failing `.py`, adds a `test_*.py` importing `unittest.mock` and adds a `.md` leaves exactly the moved destination and the added test failing, shows both diagnostics (the mock import found by the real ast-grep Python scan), and creates no entry for the `.md`.
- **AC-3:** After a violating Python edit, native commit and push attempts are refused before their marker side effects.
- **AC-4:** With python-quality disabled, without a Python marker, or without `python3` on `PATH`, the same invalid file gets no python-quality diagnostic or gate file. Two independent projects keep distinct gate state, a linked worktree is checked against its own gate file, and existing Claude Code and Codex python-quality tests stay green.
- **AC-5:** The pinned OpenCode 1.18.34 host loads the selected module in an isolated profile and demonstrates real tool bytes, model-visible diagnostics and gate state for the edit, patch and disabled scenarios; the repository quality gate passes.

## Acceptance evidence

| AC | Real input and expected result | Boundary and runnable check |
| --- | --- | --- |
| AC-1 | Git project with `pyproject.toml`, real `python3`; write `src/bad.py` with `except Exception: pass`, then edit it clean; write `notes.md` with the same text. Inspect result text and gate file. | `bun test tools/toolu-opencode/src/adapter/__tests__/python-quality-post.test.ts`; `bun run smoke:opencode-python-quality python-quality.edit`. |
| AC-2 | Seed failing `pkg/old.py` and `pkg/removed.py`, then one patch: move old to `pkg/moved.py`, delete removed, add `pkg/test_added.py` with `from unittest import mock`, add `pkg/notes.md`. Inspect files, gate keys and diagnostic. | `bun test …/python-quality-post.test.ts`; `bun run smoke:opencode-python-quality python-quality.patch`. |
| AC-3 | After the invalid write, `git commit` / `git push` (marker commands in the smoke); markers stay absent and both bash calls error with the quality-gate reason. | `bun test …/python-quality-post.test.ts`; `bun run smoke:opencode-python-quality python-quality.edit`. |
| AC-4 | Repeat the invalid write unselected, with no marker, and with a `PATH` holding only `git` and `ast-grep`; a second project; a linked worktree with committed `pyproject.toml`. | `bun test …/python-quality-post.test.ts plugins/python-quality/hooks/src/__tests__`; `bun run smoke:opencode-python-quality python-quality.disabled`. |
| AC-5 | Pinned CLI, exact SDK, isolated profile and scripted loopback provider run the three scenarios. | `bun run smoke:opencode-python-quality`, `bun run check:opencode-host`, `bun run test`. |

## Documentation impact

Update `plugins/python-quality/README.md`, `docs/python-quality/README.md`, `docs/opencode.md` (a "Python post-edit quality" section beside the TypeScript one) and `docs/opencode-host-contract.md` (name the new smoke). Regenerate the OpenCode resource mirrors those docs feed.

## Open Questions

None blocking. Whether the pinned-host smoke reaches tool execution on this machine is reported with its own result, separately from the adapter evidence.

## Spec review

**Status:** Approved. Issue criterion 1 (native edits, patches, move/delete, real toolchain) maps to AC-1, AC-2 and AC-5; criterion 2 (visible diagnostic, failing gate, commit/push refused) to AC-1 and AC-3; criterion 3 (disabled plugin, unrelated files, no mocked toolchain) to AC-4 and AC-2. Every AC names a real input, an observable result and a runnable check; the tests assert `python3` and `ast-grep` exist so a missing tool cannot pass vacuously. The no-production-change architecture rests on the probe test passing against the committed bundle. Jev judged the coverage and scope sufficient (0.78).
