use toolu_protocol::exit::Exit;

use super::{dependents_of, install_order, removable};
use crate::catalog::{Catalog, Entry, load, parse};

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
      "documentation-gate",
      "epic-orchestrator",
      "git-better",
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
  assert_eq!(order(&["epic-creator"]), ["brainstorm", "epic-creator"]);
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

fn entry(name: &str, dependencies: &[&str]) -> Entry {
  Entry {
    name: name.to_owned(),
    description: None,
    dependencies: dependencies.iter().map(|name| (*name).to_owned()).collect(),
  }
}

#[test]
fn a_dependency_cycle_is_a_usage_error_at_load_and_in_the_walk() {
  let text = r#"{"name":"t","plugins":[
    {"name":"a","dependencies":[{"name":"b"}]},
    {"name":"b","dependencies":[{"name":"a"}]}]}"#;
  let Err(loaded) = parse(text, "cycle.json") else {
    panic!("a cyclic catalog loaded");
  };
  assert_eq!(loaded.exit, Exit::Usage);
  assert!(
    loaded
      .message
      .starts_with("circular plugin dependency involving"),
    "{}",
    loaded.message
  );
  let cyclic = Catalog {
    plugins: vec![entry("a", &["b"]), entry("b", &["a"])],
  };
  let Err(walked) = install_order(&cyclic, &["a".to_owned()]) else {
    panic!("a cycle was ordered");
  };
  assert_eq!(walked.exit, Exit::Usage);
}

#[test]
fn a_dependency_outside_the_catalog_is_skipped() {
  let text = r#"{"name":"t","plugins":[{"name":"c","dependencies":[{"name":"outside"}]}]}"#;
  let Ok(catalog) = parse(text, "outside.json") else {
    panic!("the catalog did not load");
  };
  assert_eq!(
    install_order(&catalog, &["c".to_owned()]),
    Ok(vec!["c".to_owned()])
  );
}

#[test]
fn a_retired_name_is_removable_only_where_the_host_scopes_removal_to_toolu() {
  let names =
    |list: &[&str]| -> Vec<String> { list.iter().map(|name| (*name).to_owned()).collect() };
  assert_eq!(
    removable(&real(), &names(&["jev", "context7"]), true),
    Ok(())
  );
  assert_eq!(removable(&real(), &names(&["jev"]), false), Ok(()));
  for (list, scoped) in [
    (["context7"], false),
    (["../x"], true),
    (["Bad_Name"], true),
  ] {
    let Err(fail) = removable(&real(), &names(&list), scoped) else {
      panic!("{list:?} was removable");
    };
    assert_eq!(fail.exit, Exit::Usage);
    assert_eq!(
      fail.message,
      format!(
        "unknown plugin: {}. Run `toolu plugins list` for the catalog.",
        list[0]
      )
    );
  }
}
