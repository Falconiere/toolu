use std::path::Path;

use toolu_runtime::env::Env;

use super::{
  DEFAULT_MODULE_TIMEOUT, DispatchOptions, MAX_OUTPUT_BYTES, dispatch_post_tool, dispatch_pre_tool,
};

#[test]
fn invalid_config_warnings_come_first_and_an_empty_registry_says_nothing() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
  std::fs::write(dir.path().join(".claude/toolu.config.json"), "not json").unwrap();
  let home = dir.path().to_string_lossy().into_owned();
  let env = Env::from_pairs([("HOME", home)]);
  let options = DispatchOptions {
    env: &env,
    cwd: dir.path(),
    lib_dir: Path::new("/lib"),
    builtins: &[],
    rules: &[],
    selected_specs: None,
    continue_post_blocks: false,
    module_timeout: DEFAULT_MODULE_TIMEOUT,
  };
  let pre = dispatch_pre_tool("{\"tool_name\":\"Read\"}\n", &options);
  assert!(
    pre
      .result
      .stderr
      .starts_with("toolu-config: malformed JSON in "),
    "{}",
    pre.result.stderr
  );
  assert_eq!((pre.result.stdout.as_str(), pre.result.exit_code), ("", 0));
  assert_eq!(pre.trace, []);
  assert_eq!(dispatch_post_tool("{}", &options).result.stdout, "");
  assert_eq!(MAX_OUTPUT_BYTES, 8_388_608);
}
