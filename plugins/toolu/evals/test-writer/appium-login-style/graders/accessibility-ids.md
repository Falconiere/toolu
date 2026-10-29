---
type: regex
target: { source: file, path: test/login.e2e.js }
pattern: '\$\([''"`]~|accessibility.?id'
flags: i
match: contains
---
