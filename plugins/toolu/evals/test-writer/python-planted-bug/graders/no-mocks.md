---
type: regex
target: { source: file, path: test_pricing.py }
pattern: 'unittest\.mock|MagicMock|patch\('
match: not_contains
---
