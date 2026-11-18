use super::{CommandOrigin, PipelinePosition, RedirectOperator, Tristate};
use crate::analyze;

#[test]
fn every_redirect_operator_reads_back_from_its_spelling() {
  let spelled: Vec<&str> = RedirectOperator::ALL
    .iter()
    .map(|operator| operator.as_str())
    .collect();
  assert_eq!(
    spelled,
    [
      ">", ">>", "<", "<<", "<<-", "<<<", "<>", ">&", "<&", ">|", "&>", "&>>"
    ]
  );
  for operator in RedirectOperator::ALL {
    assert_eq!(RedirectOperator::parse(operator.as_str()), Some(operator));
  }
}

#[test]
fn an_unknown_spelling_is_no_operator() {
  for text in ["", ">>>", "&", "|", "<<<<"] {
    assert_eq!(RedirectOperator::parse(text), None, "{text}");
  }
}

#[test]
fn origins_and_tristates_use_the_typescript_names() {
  let origins = [
    CommandOrigin::Line,
    CommandOrigin::Substitution,
    CommandOrigin::Shell,
    CommandOrigin::Eval,
    CommandOrigin::Function,
  ];
  let names: Vec<&str> = origins.iter().map(|origin| origin.as_str()).collect();
  assert_eq!(names, ["line", "substitution", "shell", "eval", "function"]);
  let answers = [Tristate::Yes, Tristate::No, Tristate::Unknown];
  let names: Vec<&str> = answers.iter().map(|answer| answer.as_str()).collect();
  assert_eq!(names, ["yes", "no", "unknown"]);
}

#[test]
fn a_pipe_records_each_commands_index_and_the_pipeline_length() {
  let place = |source: &str| -> Vec<(String, PipelinePosition)> {
    analyze(source)
      .commands
      .iter()
      .map(|command| {
        let name = command.argv.first().cloned().flatten().unwrap_or_default();
        (name, command.pipeline)
      })
      .collect()
  };
  assert_eq!(
    place("echo alone"),
    [("echo".to_owned(), PipelinePosition::ALONE)]
  );
  assert_eq!(
    place("echo left | cat"),
    [
      ("echo".to_owned(), PipelinePosition { index: 0, size: 2 }),
      ("cat".to_owned(), PipelinePosition { index: 1, size: 2 }),
    ]
  );
}
