use super::Object;

#[test]
fn an_object_writes_its_keys_in_order_with_json_escaping() {
  let inner = Object::new().text("b", "x");
  let line = Object::new()
    .text(
      "z",
      "quote \" back \\ nl \n tab \t ctl \u{1} del \u{7f} é 😀 \u{2028} </s>",
    )
    .flag("a", false)
    .flag("t", true)
    .object("m", inner)
    .line();
  assert_eq!(
    line,
    "{\"z\":\"quote \\\" back \\\\ nl \\n tab \\t ctl \\u0001 del \u{7f} é 😀 \u{2028} </s>\",\
     \"a\":false,\"t\":true,\"m\":{\"b\":\"x\"}}\n"
  );
}

#[test]
fn an_empty_object_closes_with_or_without_a_newline() {
  assert_eq!(Object::new().line(), "{}\n");
  assert_eq!(Object::new().close(), "{}");
}
