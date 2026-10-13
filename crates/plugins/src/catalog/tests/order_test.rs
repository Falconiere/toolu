use toolu_protocol::exit::Exit;

use super::{dependents_of, install_order};
use crate::catalog::{Catalog, load, parse};

fn real() -> Catalog {
  match load(None) {
    Ok(catalog) => catalog,
    Err(fail) => panic!("the built-in catalog: {}", fail.message),
  }
}

fn order(names: &[&str]) -> Vec<String> {
  let names: Vec<String> = names.iter().map(|name| (*name).to_owned()).collect();
  match install_order(&real(), &names) {
    Ok(order) => order,
    Err(fail) => panic!("{}", fail.message),
  }
}

#[test]
fn the_declared_dependents_really_depend_on_toolu() {
  let mut dependents = dependents_of(&real(), "toolu");
  dependents.sort();
  assert_eq!(
    dependents,
    [
      "delivery-flow",
      "epic-orchestrator",
      "pr-babysit",
      "python-quality",
      "rust-quality",
      "ts-quality"
    ]
  );
}

#[test]
fn delivery_flow_and_epic_orchestrator_close_their_dependencies() {
  assert_eq!(
    order(&["delivery-flow"]),
    [
      "toolu",
      "toolu-review",
      "pr-babysit",
      "brainstorm",
      "delivery-flow"
    ]
  );
  assert_eq!(
    order(&["epic-orchestrator"]),
    [
      "toolu",
      "toolu-review",
      "pr-babysit",
      "brainstorm",
      "delivery-flow",
      "epic-orchestrator"
    ]
  );
}

#[test]
fn a_full_install_orders_every_dependent_after_toolu() {
  let catalog = real();
  let all = order(&[]);
  assert_eq!(all.len(), catalog.plugins.len());
  let core = all.iter().position(|name| name == "toolu");
  for dependent in dependents_of(&catalog, "toolu") {
    assert!(
      all.iter().position(|name| *name == dependent) > core,
      "{dependent}"
    );
  }
}

#[test]
fn a_dependent_pulls_its_dependency_first_and_repeats_appear_once() {
  assert_eq!(order(&["rust-quality"]), ["toolu", "rust-quality"]);
  assert_eq!(order(&["brainstorm"]), ["brainstorm"]);
  assert_eq!(
    order(&["toolu", "rust-quality", "toolu"]),
    ["toolu", "rust-quality"]
  );
}

#[test]
fn an_unknown_name_is_a_usage_error_naming_the_list_command() {
  for name in ["not-a-plugin", "context7"] {
    let Err(fail) = install_order(&real(), &[name.to_owned()]) else {
      panic!("{name} was ordered");
    };
    assert_eq!(fail.exit, Exit::Usage);
    assert_eq!(
      fail.message,
      format!("unknown plugin: {name}. Run `toolu plugins list` for the catalog.")
    );
  }
}

#[test]
fn a_dependency_cycle_is_a_usage_error() {
  let text = r#"{"name":"t","plugins":[
    {"name":"a","dependencies":[{"name":"b"}]},
    {"name":"b","dependencies":[{"name":"a"}]},
    {"name":"c","dependencies":[{"name":"outside"}]}]}"#;
  let catalog = match parse(text, "cycle.json") {
    Ok(catalog) => catalog,
    Err(fail) => panic!("{}", fail.message),
  };
  let Err(fail) = install_order(&catalog, &["a".to_owned()]) else {
    panic!("a cycle was ordered");
  };
  assert_eq!(fail.exit, Exit::Usage);
  assert!(
    fail
      .message
      .starts_with("circular plugin dependency involving"),
    "{}",
    fail.message
  );
  // A dependency outside the catalog is skipped, as the TypeScript walk did.
  assert_eq!(
    install_order(&catalog, &["c".to_owned()]),
    Ok(vec!["c".to_owned()])
  );
}
