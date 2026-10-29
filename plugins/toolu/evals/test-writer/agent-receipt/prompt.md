---
description: Independent tests via the test-author subagent — spec-only, and a short receipt instead of raw test output.
tags: [behavior, agent, python]
max_turns: 20
timeout_seconds: 600
allowed_tools: [Read, Glob, Grep, Skill, Agent, TodoWrite]
---

Save this module as `slug.py`. Then have a subagent write tests for it, so the tests are
independent of how it is implemented. The tests use `unittest` in `test_slug.py` and run
with `python3 -m unittest -q`. Summarize the result for me.

Spec for `slugify(title)`:

- Lowercases the title and replaces each run of non-alphanumeric characters with one hyphen.
- Trims hyphens from both ends: `slugify("Hello, World!") == "hello-world"`.
- An empty or all-punctuation title returns `""`.

```python
import re


def slugify(title):
    return re.sub(r"[^a-z0-9]+", "-", title.lower()).strip("-")
```
