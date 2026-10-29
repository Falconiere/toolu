---
type: regex
target: { source: file, path: test_duration.py }
pattern: 'unittest\.mock|MagicMock|patch\('
match: not_contains
---
