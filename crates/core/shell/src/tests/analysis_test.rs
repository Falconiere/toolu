use super::{CommandOrigin, PipelinePosition, RedirectOperator, Tristate};

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
fn alone_is_the_first_of_one() {
  assert_eq!(
    PipelinePosition::ALONE,
    PipelinePosition { index: 0, size: 1 }
  );
}
