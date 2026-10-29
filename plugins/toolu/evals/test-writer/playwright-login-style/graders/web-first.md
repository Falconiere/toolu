---
type: regex
target: { source: file, path: tests/login.spec.js }
pattern: '\.(toBeVisible|toHaveURL|toHaveText|toContainText)\('
match: contains
---
