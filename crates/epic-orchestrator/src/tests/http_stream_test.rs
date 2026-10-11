use super::cursor;

#[test]
fn status_stream_reconnect_header_wins_over_query_cursor() {
  let parsed = cursor("since=4&limit=20", Some("7")).expect("cursor");
  assert_eq!(parsed.since, 7);
  assert_eq!(parsed.limit, 20);
  assert!(cursor("limit=501", None).is_err());
  assert!(cursor("since=bad", None).is_err());
}
