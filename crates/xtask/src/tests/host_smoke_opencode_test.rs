use std::path::Path;

use super::{Opencode, argv, private_config};
use crate::host_smoke_steps::{Driver, Step};

fn repo() -> &'static Path {
  Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

#[test]
fn a_session_runs_standalone_auto_approved_with_json_output() {
  let words = argv(Path::new("/bin/opencode"), None, "do it");
  assert_eq!(words[0], "/bin/opencode");
  assert_eq!(words[1], "run");
  for flag in ["--standalone", "--auto"] {
    assert!(words.contains(&flag.to_owned()), "{flag}");
  }
  assert!(!words.contains(&"--model".to_owned()));
  assert_eq!(words.last().map(String::as_str), Some("do it"));
  let pinned = argv(Path::new("/bin/opencode"), Some("anthropic/x"), "p");
  let at = pinned.iter().position(|word| word == "--model").unwrap();
  assert_eq!(pinned[at + 1], "anthropic/x");
}

#[test]
fn the_private_config_holds_both_checkout_plugins_with_their_shims() {
  let dir = tempfile::tempdir().unwrap();
  let config = private_config(dir.path(), repo()).unwrap();
  assert_eq!(config, dir.path().join("opencode-config"));
  for name in ["toolu", "ts-quality"] {
    let plugin = config.join("opencode/plugins").join(name);
    assert!(plugin.join("hooks/hooks.json").is_file(), "{name}");
    assert!(plugin.join("hooks").is_symlink(), "{name}");
    let shim = std::fs::read_to_string(plugin.join("server.js")).unwrap();
    assert!(
      shim.contains(&format!("const PLUGIN = \"{name}\";")),
      "{shim}"
    );
  }
  let toolu = std::fs::read_to_string(config.join("opencode/plugins/toolu/server.js")).unwrap();
  assert!(toolu.contains("\"tool.execute.after\""));
  assert!(private_config(dir.path(), repo()).is_err(), "links twice");
}

#[test]
fn the_driver_names_itself_opencode_and_gets_the_staged_prompts() {
  let driver = Opencode {
    exe: "/bin/opencode".into(),
    model: Some("m/x".to_owned()),
  };
  assert_eq!(driver.name(), "opencode");
  let prompts = driver.prompts();
  assert!(prompts.stages_first);
  assert_eq!(prompts.allowed, crate::host_smoke_prompts::CODEX_ALLOWED);
  let step = Step {
    name: "edit",
    tools: "",
    prompt: "go",
  };
  let words = driver.argv(&step, Path::new("/p"));
  assert!(words.ends_with(&["--model".to_owned(), "m/x".to_owned(), "go".to_owned()]));
}

#[test]
fn a_source_server_js_is_never_written_through() {
  let dir = tempfile::tempdir().unwrap();
  let repo = dir.path().join("repo");
  let plugin = repo.join("plugins/toolu");
  std::fs::create_dir_all(plugin.join("hooks")).unwrap();
  std::fs::write(plugin.join("hooks/hooks.json"), r#"{"hooks":{}}"#).unwrap();
  std::fs::write(plugin.join("server.js"), "keep").unwrap();
  let source = repo.join("plugins/ts-quality");
  std::fs::create_dir_all(source.join("hooks")).unwrap();
  std::fs::write(source.join("hooks/hooks.json"), r#"{"hooks":{}}"#).unwrap();
  let config = private_config(&dir.path().join("evidence"), &repo).unwrap();
  assert_eq!(
    std::fs::read_to_string(plugin.join("server.js")).unwrap(),
    "keep"
  );
  let shim = config.join("opencode/plugins/toolu/server.js");
  assert!(!shim.is_symlink());
  assert!(
    std::fs::read_to_string(shim)
      .unwrap()
      .contains("const PLUGIN = \"toolu\";")
  );
}
