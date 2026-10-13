use super::load;
use crate::opencode::paths::Scope;
use crate::support::{Sandbox, TARGET, global, local, selection};

#[test]
fn the_state_reads_every_file_and_tracks_what_it_read() {
  let sb = Sandbox::new();
  let project = sb.git_project("project");
  sb.write(
    &global("opencode.json"),
    &format!("{{ \"plugin\": [\"{TARGET}\"] }}\n"),
  );
  sb.write(&local(".opencode/toolu/plugins.json"), &selection(&["jev"]));
  std::fs::create_dir_all(project.join("sub")).expect("sub");
  let state = load(&project.join("sub"), &sb.env()).expect("state");
  assert_eq!(state.installed_scopes(), Ok(vec![Scope::Global]));
  assert_eq!(state.default_selection_scope(), Scope::Project);
  assert_eq!(
    state
      .governing_selection(Scope::Project)
      .map(<[String]>::to_vec),
    Some(vec!["jev".to_owned()])
  );
  assert_eq!(state.governing_selection(Scope::Global), None);
  assert!(state.overridden_note(Scope::Global).contains("governs it"));
  assert_eq!(state.overridden_note(Scope::Project), "");
  let config = sb.path(&global("opencode.json"));
  assert_eq!(
    state.original(&config),
    Some(&Some(format!("{{ \"plugin\": [\"{TARGET}\"] }}\n")))
  );
  assert_eq!(
    state.original(&sb.path(&global("toolu/plugins.json"))),
    Some(&None)
  );
  assert!(
    state
      .original(&sb.path(&local(".opencode/toolu/plugins.json")))
      .is_some_and(Option::is_some)
  );
  assert_eq!(state.original(&sb.path("unrelated.json")), None);
}

#[test]
fn an_invalid_file_stops_the_read() {
  let sb = Sandbox::new();
  let project = sb.git_project("project");
  sb.write(&local("opencode.json"), "{ nope");
  assert!(load(&project, &sb.env()).is_err_and(|fail| fail.message.contains("is not valid JSONC")));
}
