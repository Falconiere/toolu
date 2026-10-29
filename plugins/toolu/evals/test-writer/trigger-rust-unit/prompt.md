---
description: Routing — the test-writer skill fires for this prompt.
tags: [trigger, test-writer]
max_turns: 3
allowed_tools: [Read, Glob, Grep, Skill]
---

Add unit tests for this function from src/range.rs of our Rust crate:

```rust
/// Parses "a..b" (end exclusive) into a range; a missing bound is an error.
pub fn parse_range(s: &str) -> Result<std::ops::Range<u32>, String> {
    let (a, b) = s.split_once("..").ok_or("missing ..")?;
    let start = a.parse::<u32>().map_err(|e| e.to_string())?;
    let end = b.parse::<u32>().map_err(|e| e.to_string())?;
    Ok(start..end)
}
```
