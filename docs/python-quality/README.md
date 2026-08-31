# python-quality — Python Quality Gate

**Type:** Quality Gate | **Version:** 4.10.0 | **Depends on:** `toolu`

Python `PostToolUse` quality checks registered into the toolu hook engine. Every Python file the agent edits is checked on the spot. Static-only — the gate never invokes `ruff`, `pylint`, `mypy`, or any other linter/type-checker.

## Install

```text
/plugin install python-quality@toolu
```

## What It Provides

### Post-Edit Quality Checks

Every Python file the agent edits is checked on the spot, contributing to toolu's quality gate. The checks are assembled from ordered `hooks/concerns/` fragments into one module at `SessionStart` and run only while this plugin is installed — **uninstall it and the Python rules vanish, fail-closed.**

## Checks Enforced

### 1. Size Discipline

| Limit | Default | Configurable via |
|-------|---------|-----------------|
| File line limit | 400 lines | `lang.python.maxFileLines` in toolu.config.json |
| Function line limit | 50 lines | `lang.python.maxFnLines` |

Line counting excludes blank lines and comments, matching the Rust/TypeScript gates. Function spans are tracked by indentation (Python has no braces): a `def`/`async def` opens a span at its own indent, which closes at the next non-blank line indented at or below it.

### 2. No Suppression

```python
# ❌ BANNED — bare except (swallows everything, including KeyboardInterrupt/SystemExit)
try:
    load_config()
except:
    pass

# ❌ BANNED — one-line except ...: pass (silently discarded)
try:
    load_config()
except Exception: pass

# ❌ BANNED — blanket # noqa (no :CODE suffix — suppresses every rule on the line)
x = eval(user_input)  # noqa

# ❌ BANNED — blanket # type: ignore (no [code] suffix — suppresses every mypy error)
result = risky_call()  # type: ignore

# ✅ CORRECT — scoped forms are fine
try:
    load_config()
except ConfigError as e:
    logger.error("config load failed: %s", e)
    raise

x = eval(user_input)  # noqa: S307
result = risky_call()  # type: ignore[arg-type]
```

Fix the underlying issue in code — never silence the tool. Scoped exception types and scoped `# noqa: CODE` / `# type: ignore[code]` suppressions stay legal.

### 3. Test Layout

```python
# ❌ BANNED — test-bearing file not named test_*.py or *_test.py
# validators.py
def test_email_format():
    ...

# ✅ CORRECT — colocated test_*.py next to the module it tests
# validators.py
# test_validators.py
import pytest
from validators import validate_email
```

A file is "test-bearing" if it defines a top-level `def test_...`/`async def test_...` or imports `pytest`/`unittest`. `conftest.py` and `__init__.py` are exempt. A `test_*.py` file also needs a non-test `.py` sibling in the same directory — otherwise there's nothing for it to be colocated with.

### 4. No Mocks (test files only)

```python
# ❌ BANNED — in test_*.py / *_test.py / conftest.py
from unittest.mock import MagicMock, patch

def test_process(mocker):
    mocker.patch("app.send_email")

# ✅ CORRECT — real data, real code paths
def test_process(tmp_path):
    result = process(real_fixture_file(tmp_path))
    assert result.status == "ok"
```

Blocks `unittest.mock`/`mock`/`pytest_mock` imports (covering `MagicMock`) plus the `mocker`/`monkeypatch` fixtures in test files. Opt out per-project via `lang.python.noMocks` (default `true`).

### 5. Docstrings (advisory, non-blocking)

```python
# ✅ CORRECT — public def/class opens with a docstring
def validate_token(token: str) -> Claims:
    """Validate the incoming JWT and return its claims."""
    ...

# ⚠️ ADVISORY — public def/class with no docstring
def validate_token(token: str) -> Claims:
    ...
```

Top-level, non-underscore-prefixed `def`/`class` should open with a docstring. This check is **advisory only** — it surfaces as guidance, never fails the gate.

## How the Gate Works

1. **Agent edits a `.py` file** — `Write` or `Edit` tool call
2. **PostToolUse hook fires** — the assembled module checks the file
3. **Violation found** (size, suppression, test layout, mocks) → gate goes **failing**, new task blocked until fixed
4. **Fix the violation** → gate clears, continue working

Docstring findings never flip the gate — they ride along as advisory context only. The gate is **multi-slot**: a failing test command and a failing file check are tracked independently, so fixing one never silently masks the other.

## Configuration

Configure thresholds per project in `toolu.config.json`:

```json
{
  "version": 1,
  "lang": {
    "python": {
      "maxFileLines": 500,
      "maxFnLines": 60,
      "noMocks": true
    }
  }
}
```

Precedence: project/user override → built-in default (400/50). A value of `0` or `"off"` means "no override" and falls through.

## Usage Example

```text
# Session with the python-quality plugin installed:

User: "Add a config loader to app/config.py"
Agent: *writes the function*

> PostToolUse: Checking app/config.py...
> ❌ Gate: FAILING
>   - app/config.py:18: bare except: (swallows everything)
>   - app/config.py:40: function load_config exceeds 50-line limit (68 lines)

# Agent is BLOCKED from starting new tasks until these are fixed:
#   - Replace bare except with a specific exception type
#   - Split load_config into smaller functions

> PostToolUse: Checking app/config.py...
> ✅ Gate: PASSING
```

## Hooks

The fragments register into the core toolu dispatcher and run only while this plugin is installed. Uninstalling immediately removes the Python rules:

```text
/plugin uninstall python-quality@toolu
# → All Python checks stop firing on the next edit
```
