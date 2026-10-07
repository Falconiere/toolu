use toolu_runtime::env::Env;

#[test]
fn a_timestamp_outside_digits_is_rejected() {
  let env = Env::from_pairs([("TOOLU_TIMESTAMP", "../x")]);
  let err = super::backup_stamp(&env).unwrap_err();
  assert_eq!(err, "invalid backup timestamp: ../x");
}

#[test]
fn unix_epoch_is_the_first_civil_day() {
  assert_eq!(super::civil_date(0), (1970, 1, 1));
}
