//! The crate links from outside and Muse payloads reach the native walk.

use std::time::Duration;

use toolu_engine::{DispatchOptions, dispatch_post_tool, dispatch_pre_tool};
use toolu_runtime::env::Env;

fn assert_muse_refusal(
  body: &str,
  options: &DispatchOptions<'_>,
  pre_reason: &str,
  post_reason: &str,
) {
  for (result, reason) in [
    (dispatch_pre_tool(body, options), pre_reason),
    (dispatch_post_tool(body, options), post_reason),
  ] {
    let expected = format!(
      "{}\n",
      serde_json::json!({"decision":"block","reason":reason})
    );
    assert_eq!(result.result.stdout, expected);
    assert_eq!(result.result.stderr, "");
    assert_eq!(result.result.exit_code, 0);
    assert_eq!(result.trace, Vec::new());
  }
}

#[test]
fn the_crate_links_and_names_its_layer() {
  assert_eq!(toolu_engine::LAYER, "engine");
}

#[test]
fn muse_dispatch_accepts_valid_tools_and_refuses_malformed_payloads() {
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
    quality_rules: &[],
    selected_specs: None,
    continue_post_blocks: false,
    split_by_caller: false,
    module_timeout: Duration::from_secs(1),
  };
  let body = r#"{"tool_name":"Bash","tool_input":{"command":"ls"},"session_id":"s1"}"#;
  let pre = dispatch_pre_tool(body, &options);
  assert_eq!(pre.result.stdout, "");
  assert_eq!(pre.result.exit_code, 0);
  assert_eq!(pre.result.stderr, "");
  assert_eq!(pre.trace, Vec::new());
  let post = dispatch_post_tool(body, &options);
  assert_eq!(post.result.stdout, "");
  assert_eq!(post.result.exit_code, 0);
  assert_eq!(post.result.stderr, "");
  assert_eq!(post.trace, Vec::new());

  assert_muse_refusal(
    "{invalid",
    &options,
    "toolu: hook payload is not JSON",
    "toolu: hook payload is not JSON",
  );
  assert_muse_refusal(
    r#"{"tool_name":"Write","tool_input":{"path":""}}"#,
    &options,
    "Unable to parse apply_patch file headers; patch blocked so protected-file and quality gates cannot be bypassed.",
    "Unable to parse apply_patch file headers; per-file post-edit quality checks could not run.",
  );
}
