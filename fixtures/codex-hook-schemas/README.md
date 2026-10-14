# Codex hook output schemas

Verbatim copies of `codex-rs/hooks/schema/generated/<event>.command.output.schema.json`
from [openai/codex](https://github.com/openai/codex) at commit
`fe50d010e203a9b8dda2c7737d7d8e4a80e6ab44` (fetched 2026-09-28). Codex validates
command-hook stdout against these. `crates/core/protocol/tests/encode_fixture.rs`
checks every payload the host encoders emit against them, and
`crates/cli/tests/helpers/missing.rs` checks the launcher's own message. Refresh by
re-fetching the same paths at a newer commit and updating the SHA above.
