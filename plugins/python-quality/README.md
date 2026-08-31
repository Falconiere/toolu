# python-quality

Python `PostToolUse` quality checks registered into the toolu hook engine.

## Install

```
/plugin install python-quality@toolu
```

Requires the `toolu` plugin.

## What it provides

Every Python file the agent edits is checked on the spot, contributing to toolu's quality gate. The checks (assembled from ordered `hooks/concerns/` fragments into one module at `SessionStart`) are static-only — the gate never invokes `ruff`, `pylint`, or any other linter:

- File / function line limits (config-driven).
- No suppression: bare `except:`, one-line `except ...: pass`, blanket `# noqa`, bare `# type: ignore`.
- Colocated `test_*.py` next to every module it tests.
- No mocks in tests (`unittest.mock`/`mock`/`pytest_mock` imports — covering `MagicMock` — plus `mocker`/`monkeypatch` fixtures).
- Docstring checks on public functions and classes.

The fragments register into the core toolu dispatcher and run only while this plugin is installed — uninstall it and the Python rules vanish, fail-closed.
