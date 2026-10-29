---
type: regex
target: { source: file, path: test/login.e2e.js }
pattern: '\.pause\(|setTimeout\(|sleep\('
match: not_contains
---
