//! The crate links from outside and Muse dispatch remains fail closed until
//! #545 ports the walk to Muse tool calls.

use std::time::Duration;

use toolu_engine::{DispatchOptions, dispatch_post_tool, dispatch_pre_tool};
use toolu_runtime::env::Env;

#[test]
fn the_crate_links_and_names_its_layer() {
  assert_eq!(toolu_engine::LAYER, "engine");
}

#[test]
fn muse_dispatch_fails_closed_until_its_walk_is_ported() {
  let dir = tempfile::tempdir().unwrap();
  let home = dir.path().to_str().unwrap();
  let env = Env::from_pairs([
    ("HOME", home),
    ("MUSE_PLUGIN_ID", "toolu"),
    ("TOOLU_CONFIG_DIR", home),
  ]);
  let options = DispatchOptions {
    env: &env,
    cwd: dir.path(),
    lib_dir: dir.path(),
    builtins: &[],
    rules: &[],
    selected_specs: None,
    continue_post_blocks: false,
    split_by_caller: false,
    module_timeout: Duration::from_secs(1),
  };
  let body = r#"{"tool_name":"Bash","tool_input":{"command":"ls"},"session_id":"s1"}"#;
  let pre = dispatch_pre_tool(body, &options);
  assert_eq!(pre.result.stdout, "");
  assert_eq!(pre.result.exit_code, 2);
  assert_eq!(
    pre.result.stderr,
    "blocked: toolu PreToolUse dispatcher failed: Muse tool calls are not gated yet (#545)\n"
  );
  assert_eq!(pre.trace, Vec::new());
  let post = dispatch_post_tool(body, &options);
  assert_eq!(post.result.stdout, "");
  assert_eq!(post.result.exit_code, 2);
  assert_eq!(
    post.result.stderr,
    "toolu PostToolUse dispatcher failed: Muse tool calls are not gated yet (#545)\n"
  );
  assert_eq!(post.trace, Vec::new());
}
