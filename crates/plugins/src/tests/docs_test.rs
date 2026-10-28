//! The installer guide (`docs/cli/installer.md`) describes the picker, the
//! prompt-free update and the terminal output, and no longer the behavior they
//! replaced.

/// The guide with every run of whitespace made one space, so a phrase that
/// wraps across lines still matches.
fn guide() -> String {
  let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/cli/installer.md");
  let text = std::fs::read_to_string(path).expect("docs/cli/installer.md");
  text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn the_guide_documents_hosts_update_and_terminal_output() {
  let guide = guide();
  for phrase in [
    "## Doctor",
    "`toolu plugins doctor` diagnoses plugin install health",
    "## Choosing hosts",
    "## Updating every host",
    "## Terminal output",
    "↑/↓ (or `k`/`j`) move, space toggles, `a` toggles all, Enter confirms",
    "Cursor when its config directory (`~/.cursor`) exists or `cursor-agent` is on `PATH`",
    "`toolu plugins update` never asks which host",
    "`update` without `--host` prints the `install` document",
    "A script that reads one host's steps should pass `--host`",
    "`NO_COLOR` (non-empty) drops color but keeps the rails",
    "verified with 2.1.311",
    "names each host whose listing could not be read",
    "its listing cannot be read (`could not list its plugins: <cause>`)",
    "followed by `; toolu needs Claude Code 2.1.311 or newer`",
  ] {
    assert!(guide.contains(phrase), "installer.md lacks: {phrase}");
  }
}

#[test]
fn the_guide_no_longer_describes_what_was_replaced() {
  let guide = guide();
  for stale in [
    "Cursor is never probed",
    "The prompts are numbered lines on the terminal",
    "`remove` and `update` print",
    "No live progress bar",
    "`list`, `remove`, and `update` ask for one host",
  ] {
    assert!(!guide.contains(stale), "installer.md still says: {stale}");
  }
}
