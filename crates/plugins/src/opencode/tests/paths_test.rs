use toolu_runtime::env::Env;

use super::{Scope, paths, project_root_of};
use crate::support::Sandbox;

#[test]
fn global_files_project_files_and_selections() {
  let sb = Sandbox::new();
  let root = sb.git_project("project");
  let env = sb.env();
  let p = paths(&root, &env, &root);
  let global = sb.path("home/.config/opencode");
  assert_eq!(p.global_files, [global.join("config.json"), global.join("opencode.json"), global.join("opencode.jsonc")]);
  assert_eq!(p.project_files[3], root.join(".opencode/opencode.jsonc"));
  assert_eq!(p.selection(Scope::Global), global.join("toolu/plugins.json"));
  assert_eq!(p.selection(Scope::Project), root.join(".opencode/toolu/plugins.json"));
  assert_eq!(p.toolu_config, root.join(".opencode/toolu.config.json"));
  assert_eq!((Scope::Global.name(), Scope::Project.name()), ("global", "project"));
}

#[test]
fn the_selection_root_follows_the_overrides_but_not_an_inherited_data_root() {
  let sb = Sandbox::new();
  let root = sb.path("p");
  let custom = sb.env().with("TOOLU_CONFIG_DIR", "/custom");
  assert_eq!(paths(&root, &custom, &root).selection_global, std::path::Path::new("/custom/toolu/plugins.json"));
  let inherited = custom.with("TOOLU_OPENCODE_DATA_ROOT", "/custom").with("TOOLU_OPENCODE_HOME", "/home-oc");
  assert_eq!(paths(&root, &inherited, &root).selection_global, std::path::Path::new("/home-oc/toolu/plugins.json"));
  let relative = Env::from_pairs([("HOME", "/h"), ("XDG_CONFIG_HOME", "cfg")]);
  assert_eq!(paths(&root, &relative, &root).global_files[0], root.join("cfg/opencode/config.json"));
}

#[test]
fn the_project_root_is_the_git_worktree_even_from_a_subdirectory() {
  let sb = Sandbox::new();
  let root = sb.git_project("project");
  let sub = root.join("a/b");
  std::fs::create_dir_all(&sub).expect("sub");
  assert_eq!(project_root_of(&sub, &sb.env()), root);
  let loose = sb.path("loose");
  std::fs::create_dir_all(&loose).expect("loose");
  assert_eq!(project_root_of(&loose, &sb.env()), loose);
}
