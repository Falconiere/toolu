//! An epic's GitHub watch: a changed epic issue arms a refresh of that epic.

use toolu_http_test_support::{Fixture, Reply};

use super::fixture::{engine, graphql_body, rest_routes};
use crate::epics::Epic;
use crate::model::Step;
use crate::watch;

#[test]
fn an_epic_change_arms_a_refresh_of_that_epic() {
  let fixture = Fixture::start().expect("fixture");
  rest_routes(&fixture);
  fixture
    .route("/graphql", Reply::new(200, graphql_body(false)))
    .expect("GraphQL route");
  fixture
    .route(
      "/repos/o/r/issues/9/sub_issues?per_page=100",
      Reply::new(200, "[]").header("ETag", "\"subs\""),
    )
    .expect("sub-issues route");
  let (temp, mut engine, tick) = engine(&fixture);
  let epic = Epic::new("epic", &temp.path().display().to_string(), 0, None);
  engine.world.epics.insert("epic".into(), epic);
  engine.world.epic_refs.insert("epic".into(), "o/r#9".into());
  watch::sync(&mut engine.world);
  engine.github_tick(&tick);
  assert!(engine.world.outbox.iter().any(|step| matches!(step, Step::Journal(row) if row.name == "github-epic-changed" && row.key == "epic")));
  let pending = engine.world.epics["epic"].pending.clone().expect("armed");
  assert_eq!(pending.action, crate::model::Action::Refresh);
}
