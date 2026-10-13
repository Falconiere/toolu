# python-quality

Python `PostToolUse` quality checks compiled into `toolu` and enabled by a registry manifest.

## Install

**Prerequisite:** the native [`toolu` binary](../../docs/install.md).

```
/plugin install python-quality@toolu
```

Requires the `toolu` plugin.

## What it provides

Every Python file the agent edits is checked on the spot, contributing to toolu's quality gate. The checks are the `toolu-python-quality` crate (`crates/python-quality`), compiled into `toolu`. The `SessionStart` register hook publishes its manifest into toolu's registry, plus a small `.sh` bridge that lets toolu's current post-tools dispatcher run the same rule until #425, and removes the module of earlier releases. The checks are static-only — the gate never invokes `ruff`, `pylint`, or any other linter:

- File / function line limits (config-driven).
- No suppression: bare `except:`, one-line `except ...: pass`, blanket `# noqa`, bare `# type: ignore`.
- Colocated `test_*.py` next to every module it tests.
- No mocks in tests (`unittest.mock`/`mock`/`pytest_mock` imports — covering `MagicMock` — plus `mocker`/`monkeypatch` fixtures).
- Docstring checks on public functions and classes.

The rule runs inside the core toolu dispatcher only while this plugin is installed — uninstall it and the Python rules vanish, fail-closed. `toolu python-quality --help` lists the checks, and `toolu doctor` reports whether the rule is registered.

## OpenCode

OpenCode support is withdrawn. The notes below are not a supported install path.

Add `python-quality` to the project selection and restart OpenCode:

```json
{ "version": 1, "enabled": ["toolu", "python-quality"] }
```

The project must be a git repository with `pyproject.toml`, `setup.py`, `setup.cfg`, or `requirements.txt` at its root, and `python3` must be on `PATH`. Completed `write`, `edit`, and `apply_patch` calls check changed `.py` files. The installed `ast-grep` CLI runs the no-mocks scan with its Python parser; without it, that rule is skipped. A multi-file patch checks every changed Python destination and clears prior entries for deleted files and moved sources. Other extensions and disabled plugins do not run these checks. Linked worktrees are checked, each against its own gate file.

Violations are appended to the completed tool result and recorded in the project's `.opencode/tmp/quality-gate-status.json`; they do not undo the edit. A failing entry blocks later commit and push attempts until a clean edit or deletion clears it. The pinned-host proof is the OpenCode python-quality smoke in the toolu checkout; see [OpenCode Python post-edit quality](../../docs/opencode.md#python-post-edit-quality).
