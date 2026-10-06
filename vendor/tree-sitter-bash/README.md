# tree-sitter-bash 0.23.3, vendored

The Bash grammar `toolu-shell` parses with (#416). The files come from the crates.io release 0.23.3, upstream commit `487734f87fd87118028a65a4599352fa99c9cde8`. They are unchanged except for three things:

- the scanner fix below;
- the two Rust binding files are formatted with the workspace's `rustfmt.toml`, which `cargo fmt --all` applies to path dependencies too;
- `Cargo.toml` is written by hand and declares its own workspace root, so cargo neither treats the crate as a member nor looks for a workspace above it.

The fix: `src/scanner.c` `serialize` carries the bounds check of tree-sitter-bash 0.25.1. The 0.23.3 check leaves out the 4-byte delimiter length, so a heredoc state of 1,025 to 1,027 bytes fails tree-sitter's `length <= 1024` assertion or overruns its buffer. Fuzzing `toolu-shell` found both. tree-sitter-bash 0.25.1 needs tree-sitter 0.25 or later, which `deny.toml` does not admit yet (`docs/shell-analysis.md`).

Remove this directory, and depend on the crates.io release again, once the workspace can use a tree-sitter-bash with the fix.
