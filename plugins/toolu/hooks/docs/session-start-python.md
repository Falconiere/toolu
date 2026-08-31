## Python notes
- Tests colocated: `test_<module>.py` beside the module — real data, no `unittest.mock`/`pytest-mock`.
- No bare `except:`, blanket `# noqa`, or bare `# type: ignore` — fix the cause.
