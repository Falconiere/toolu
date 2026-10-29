---
type: regex
target: { source: file, path: test/login.e2e.js }
pattern: 'xpath|\$\([''"`]//'
flags: i
match: not_contains
---
