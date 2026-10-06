//! Malformed input (#416 AC-3): the commands read before a syntax error are
//! still reported, and any syntax error makes the line unknown, since
//! tree-sitter-bash reports ERROR nodes for valid bash too.

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
    assert!(analysis.unknown, "{source}");
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

/// Valid bash tree-sitter-bash 0.23 reads with an ERROR node: the commands
/// around it may be merged or missing, so the line fails closed (pre-push
/// review of #416).
#[test]
fn valid_bash_tree_sitter_cannot_read_is_unknown() {
  for source in [
    "cat <<'EOF'; git push\nx\nEOF",
    "cat <<EOF >f; rm -rf x\nx\nEOF\n",
    "3<<EOF git push\nx\nEOF\n",
    "echo \"a``\" > .env",
    "! while true; do git push; done",
    "{>f git push;}",
    "coproc { git push; }",
    // `[` ends at the newline in bash; tree-sitter reads the next lines as its arguments.
    "[\nrm -rf x\n]",
    // A body line that starts with `\` is read into the delimiter's line.
    "bash <<'EOF'\n\\x\ngit push\nEOF",
    "bash <<EOF\n\\\ngit push\nEOF",
    "bash <<EOF > out\n\\git push\nEOF",
  ] {
    assert!(analyze(source).unknown, "{source:?}");
  }
}

#[test]
fn two_backtick_substitutions_read_as_one_are_unknown() {
  let analysis = analyze("echo `ls` `git push`");
  assert!(analysis.unknown);
  let messages: Vec<&str> = analysis
    .errors
    .iter()
    .map(|error| error.message.as_str())
    .collect();
  assert_eq!(messages, ["backticks: two substitutions read as one"]);
}
