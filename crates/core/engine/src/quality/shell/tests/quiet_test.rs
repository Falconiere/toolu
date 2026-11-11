use super::read_only;

#[test]
fn read_only_classifier_rejects_mutating_git_and_cargo_fix() {
  for (line, expected) in [
    ("cat x.rs", true),
    ("git status", true),
    ("git checkout -- x.rs", false),
    ("cargo clippy", true),
    ("cargo clippy --fix", false),
  ] {
    let analysis = toolu_shell::analyze(line);
    assert!(!analysis.unknown, "{line}");
    assert_eq!(analysis.commands.len(), 1, "{line}");
    assert_eq!(read_only(&analysis.commands[0]), expected, "{line}");
  }
}
