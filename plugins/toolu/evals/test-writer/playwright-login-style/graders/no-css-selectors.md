---
type: regex
target: { source: file, path: tests/login.spec.js }
pattern: 'locator\([''"`][.#\[]|xpath='
match: not_contains
---
