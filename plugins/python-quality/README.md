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
