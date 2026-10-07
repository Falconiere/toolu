use super::{missing_bun, not_run};

#[test]
fn the_missing_bun_messages_name_the_modules_and_the_install() {
  let deny = missing_bun("x@t__m.js");
  assert!(
    deny.starts_with("toolu-registry: module x@t__m.js needs Bun 1.4.x"),
    "{deny}"
  );
  assert!(deny.ends_with("install it from https://bun.sh or remove the module"));
  assert_eq!(
    not_run(&["a.js", "b.js"]),
    "toolu-registry: 2 registry module(s) did not run because Bun was not found: a.js, b.js. Install Bun 1.4.x from https://bun.sh."
  );
}
