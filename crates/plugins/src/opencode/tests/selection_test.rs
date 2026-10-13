use super::{closure, dependents_blocking, effective_enabled, read_selection_file, selection_text, skills_disabled};
use crate::support::{Sandbox, catalog, names};

fn sorted(mut list: Vec<String>) -> Vec<String> {
  list.sort();
  list
}

#[test]
fn absent_is_none_and_invalid_files_fail_closed_naming_the_file() {
  let sb = Sandbox::new();
  assert_eq!(read_selection_file(&sb.path("none.json")).map(|file| file.enabled), Ok(None));
  for (name, text) in [
    ("bad.json", "{ not json"),
    ("extra.json", r#"{"version":1,"enabled":[],"host":"x"}"#),
    ("v2.json", r#"{"version":2,"enabled":[]}"#),
    ("list.json", "[]"),
    ("names.json", r#"{"version":1,"enabled":[1]}"#),
    ("noenabled.json", r#"{"version":1}"#),
  ] {
    let path = sb.write(name, text);
    let Err(fail) = read_selection_file(&path) else {
      panic!("{name} was read");
    };
    assert_eq!(fail.exit.code(), 1);
    assert!(fail.message.contains(&path.display().to_string()), "{}", fail.message);
    assert!(fail.message.ends_with("; nothing was written"), "{}", fail.message);
  }
  let ok = sb.write("ok.json", &selection_text(&names(&["jev"])));
  assert_eq!(read_selection_file(&ok).map(|file| file.enabled), Ok(Some(names(&["jev"]))));
  assert!(read_selection_file(&sb.path("home")).is_err_and(|fail| fail.message.starts_with("cannot read")));
}

#[test]
fn selection_text_is_what_json_stringify_writes() {
  assert_eq!(selection_text(&names(&["jev", "toolu"])), "{\n  \"version\": 1,\n  \"enabled\": [\n    \"jev\",\n    \"toolu\"\n  ]\n}\n");
  assert_eq!(selection_text(&[]), "{\n  \"version\": 1,\n  \"enabled\": []\n}\n");
}

#[test]
fn closure_over_the_real_catalog_adds_every_dependency_and_drops_unknown_names() {
  let catalog = catalog();
  assert_eq!(
    sorted(effective_enabled(&catalog, Some(&names(&["delivery-flow"])), &[])),
    ["brainstorm", "delivery-flow", "pr-babysit", "toolu", "toolu-review"]
  );
  assert_eq!(effective_enabled(&catalog, None, &[]).len(), catalog.plugins.len());
  assert_eq!(effective_enabled(&catalog, Some(&names(&["jev", "not-a-plugin"])), &[]), ["jev"]);
  assert_eq!(closure(&catalog, &names(&["nope"])), Vec::<String>::new());
}

#[test]
fn skills_false_drops_a_listed_plugin_but_closure_re_adds_a_needed_one() {
  let sb = Sandbox::new();
  let config = sb.write(".opencode/toolu.config.json", r#"{"version":1,"skills":{"jev":false,"pr-babysit":false,"ast-grep":true}}"#);
  let disabled = sorted(skills_disabled(&config));
  assert_eq!(disabled, ["jev", "pr-babysit"]);
  let enabled = effective_enabled(&catalog(), Some(&names(&["jev", "pr-babysit", "delivery-flow"])), &disabled);
  assert!(!enabled.contains(&"jev".to_owned()));
  assert!(enabled.contains(&"pr-babysit".to_owned()));
  for bad in [r#"{"skills":{"jev":"no"}}"#, "not json", r#"{"skills":[]}"#] {
    let path = sb.write("bad.json", bad);
    assert_eq!(skills_disabled(&path), Vec::<String>::new(), "{bad}");
  }
  assert_eq!(skills_disabled(&sb.path("none.json")), Vec::<String>::new());
}

#[test]
fn dependents_block_a_removal_unless_they_are_removed_too() {
  let catalog = catalog();
  let enabled = effective_enabled(&catalog, Some(&names(&["delivery-flow"])), &[]);
  assert_eq!(dependents_blocking(&catalog, &enabled, &names(&["pr-babysit"]), "pr-babysit"), ["delivery-flow"]);
  assert_eq!(dependents_blocking(&catalog, &enabled, &names(&["pr-babysit", "delivery-flow"]), "pr-babysit"), Vec::<String>::new());
}
