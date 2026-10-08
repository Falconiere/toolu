use super::*;

#[test]
fn running_step_wins_over_next() {
  let ledger = Ordered::parse(r#"{"steps":[{"id":"one","status":"running","model":"haiku"},{"id":"two","model":"opus"}],"next":"two"}"#).unwrap();
  assert_eq!(step_join(&ledger), ("one".to_owned(), "haiku".to_owned()));
}
