use crate::Reply;
use crate::routes::Routes;

fn statuses(routes: &mut Routes, path: &str, count: usize) -> Vec<Option<u16>> {
  (0..count)
    .map(|_| routes.next(path, b"").map(|reply| reply.status))
    .collect()
}

#[test]
fn a_sequence_is_served_in_order_and_its_last_reply_repeats() {
  let mut routes = Routes::default();
  routes.set(
    "/p",
    vec![
      Reply::new(500, ""),
      Reply::new(502, ""),
      Reply::new(200, ""),
    ],
  );
  assert_eq!(
    statuses(&mut routes, "/p", 5),
    [Some(500), Some(502), Some(200), Some(200), Some(200)]
  );
}

#[test]
fn setting_a_path_again_replaces_its_sequence() {
  let mut routes = Routes::default();
  routes.set("/p", vec![Reply::new(500, ""), Reply::new(200, "")]);
  routes.set("/p", vec![Reply::new(304, "")]);
  assert_eq!(statuses(&mut routes, "/p", 2), [Some(304), Some(304)]);
}

#[test]
fn an_unknown_path_or_an_empty_sequence_has_no_reply() {
  let mut routes = Routes::default();
  assert_eq!(routes.next("/none", b""), None);
  routes.set("/p", vec![Reply::new(200, "")]);
  routes.set("/p", Vec::new());
  assert_eq!(routes.next("/p", b""), None);
}

#[test]
fn a_body_needle_answers_first_and_other_bodies_fall_back_to_the_path() {
  let mut routes = Routes::default();
  routes.set("/graphql", vec![Reply::new(200, "")]);
  routes.set_when("/graphql", "\"n\":7", vec![Reply::new(201, "")]);
  let status = |routes: &mut Routes, body: &[u8]| routes.next("/graphql", body).map(|r| r.status);
  assert_eq!(status(&mut routes, br#"{"n":7}"#), Some(201));
  assert_eq!(status(&mut routes, br#"{"n":8}"#), Some(200));
  routes.set_when("/graphql", "\"n\":7", Vec::new());
  assert_eq!(status(&mut routes, br#"{"n":7}"#), Some(200));
  assert_eq!(routes.next("/other", br#"{"n":7}"#), None);
}
