---
type: regex
target: { source: file, path: tests/login.spec.js }
pattern: 'waitForTimeout|setTimeout\('
match: not_contains
---
