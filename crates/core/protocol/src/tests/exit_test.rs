use super::Exit;

#[test]
fn each_exit_has_its_documented_code_and_name() {
  let table: Vec<(u8, &str)> = Exit::ALL
    .iter()
    .map(|exit| (exit.code(), exit.name()))
    .collect();
  assert_eq!(
    table,
    [
      (0, "success"),
      (1, "failure"),
      (2, "blocked"),
      (64, "usage"),
      (69, "unavailable"),
      (75, "tempfail"),
    ]
  );
}

#[test]
fn every_exit_explains_itself_on_one_line() {
  for exit in Exit::ALL {
    let meaning = exit.meaning();
    assert!(!meaning.is_empty(), "{exit:?}");
    assert!(!meaning.contains('\n'), "{exit:?}");
  }
  assert_eq!(
    Exit::Blocked.meaning(),
    "a hook or gate blocked or denied the action"
  );
}
