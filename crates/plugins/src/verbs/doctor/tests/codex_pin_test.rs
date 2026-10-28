use super::notice;
use toolu_runtime::env::Env;

fn env_with_config(body: &str) -> (tempfile::TempDir, Env) {
  let root = tempfile::tempdir().expect("tempdir");
  let home = root.path().join(".codex");
  std::fs::create_dir_all(&home).expect("mkdir");
  std::fs::write(home.join("config.toml"), body).expect("write");
  let env = Env::from_pairs([
    ("HOME", root.path().to_str().expect("utf8")),
    ("CODEX_HOME", home.to_str().expect("utf8")),
  ]);
  (root, env)
}

#[test]
fn notice_follows_the_marketplace_ref() {
  let (_keep, wrong) = env_with_config(
    r#"
[marketplaces.toolu]
source_type = "git"
source = "https://github.com/Falconiere/toolu.git"
ref = "v8.5.0"
"#,
  );
  let text = notice(&wrong, "8.6.0").expect("wrong ref");
  assert!(text.contains("v8.5.0"), "{text}");
  assert!(text.contains("toolu plugins update --host codex"), "{text}");

  let (_keep, pinned) = env_with_config(
    r#"
[marketplaces.toolu]
source_type = "git"
source = "https://github.com/Falconiere/toolu.git"
ref = "v8.6.0"
"#,
  );
  assert_eq!(notice(&pinned, "8.6.0"), None);
}
