//! Malformed input (#416 AC-3): the commands read before a syntax error are
//! still reported, and a line with no command at all is unknown.

use toolu_shell::analysis::Tristate;
use toolu_shell::analyze;
use toolu_shell::git::runs_git_subcommand;

#[test]
fn a_push_before_a_syntax_error_is_still_a_push() {
  for source in [
    "git push origin main; echo \"unterminated",
    "git push origin main\necho 'x",
    "git push origin main && echo $(",
  ] {
    let analysis = analyze(source);
    assert!(!analysis.errors.is_empty(), "{source}");
    assert!(!analysis.unknown, "{source}");
    assert_eq!(
      runs_git_subcommand(&analysis, "push"),
      Tristate::Yes,
      "{source}"
    );
    assert!(
      analysis.commands.iter().all(|command| !command.exit_proves),
      "{source}"
    );
  }
}

#[test]
fn a_line_with_no_command_is_unknown() {
  for source in [")", "if", "fi", "case", "esac x"] {
    let analysis = analyze(source);
    assert!(analysis.unknown, "{source}");
    assert_eq!(
      runs_git_subcommand(&analysis, "push"),
      Tristate::Unknown,
      "{source}"
    );
  }
}
