use super::read;

#[test]
fn malformed_payload_uses_the_renderer_defaults() {
  let found = read(Some("not json {{{"));
  assert_eq!(found.model, "Claude");
  assert_eq!(found.ctx_size, "0");
  assert_eq!(found.ctx_used, "0");
}

#[test]
fn wrong_type_stops_later_fields_but_keeps_earlier_ones() {
  let found = read(Some(
    r#"{"model":{"display_name":"Opus"},"effort":1,"cwd":"/repo","context_window":{"context_window_size":200000}}"#,
  ));
  assert_eq!(found.model, "Opus");
  assert_eq!(found.cwd, "");
  assert_eq!(found.ctx_size, "0");
}
