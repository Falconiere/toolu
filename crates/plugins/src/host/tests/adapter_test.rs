use super::{Adapter, Installed};
use crate::args::Scope;

fn words(argv: &[String]) -> String {
  argv.join(" ")
}

#[test]
fn claude_builds_marketplace_install_with_scope_remove_and_update() {
  let claude = Adapter::Claude;
  assert_eq!(
    words(&claude.add_marketplace(Some("v8.2.0"))),
    "claude plugin marketplace add Falconiere/toolu"
  );
  assert_eq!(
    words(&claude.install("rust-quality", Some(Scope::User))),
    "claude plugin install rust-quality@toolu --scope user"
  );
  assert_eq!(
    words(&claude.install("rust-quality", None)),
    "claude plugin install rust-quality@toolu"
  );
  assert_eq!(
    words(&claude.remove("ast-grep")),
    "claude plugin uninstall ast-grep@toolu"
  );
  assert_eq!(
    words(&claude.update("ast-grep")),
    "claude plugin update ast-grep@toolu"
  );
  assert_eq!(words(&claude.list_installed()), "claude plugin list --json");
  assert_eq!(
    words(&claude.list_available()),
    "claude plugin list --available --json"
  );
}

#[test]
fn codex_omits_scope_and_re_adds_to_update() {
  let codex = Adapter::Codex;
  assert_eq!(
    words(&codex.install("rust-quality", Some(Scope::User))),
    "codex plugin add rust-quality@toolu"
  );
  assert_eq!(
    words(&codex.update("ast-grep")),
    "codex plugin add ast-grep@toolu"
  );
  assert_eq!(
    words(&codex.remove("ast-grep")),
    "codex plugin remove ast-grep@toolu"
  );
  assert_eq!(
    words(&codex.add_marketplace(Some("v8.2.0"))),
    "codex plugin marketplace add Falconiere/toolu --ref v8.2.0"
  );
  assert_eq!(
    words(&codex.add_marketplace(None)),
    "codex plugin marketplace add Falconiere/toolu"
  );
}

#[test]
fn listings_normalize_and_the_wrong_shape_is_an_error_naming_the_command() {
  let claude =
    Adapter::Claude.parse_list(r#"[{"id":"jev@toolu","version":"7.11.0","enabled":false}]"#);
  assert_eq!(
    claude,
    Ok(vec![Installed {
      name: "jev".to_owned(),
      version: "7.11.0".to_owned(),
      enabled: false,
      source: None
    }])
  );
  let Err(wrong) = Adapter::Claude.parse_list(r#"{"installed":[]}"#) else {
    panic!("an object parsed as a claude listing");
  };
  assert!(wrong.starts_with("claude plugin list --json: "), "{wrong}");
  let Err(wrong) = Adapter::Codex.parse_list("[]") else {
    panic!("an array parsed as a codex listing");
  };
  assert!(wrong.starts_with("codex plugin list --json: "), "{wrong}");
}

#[test]
fn muse_builds_its_plugins_argv_with_json_and_no_scope() {
  let muse = Adapter::Muse;
  assert_eq!(
    words(&muse.install("toolu", Some(Scope::User))),
    "muse plugins install toolu@toolu --json"
  );
  assert_eq!(words(&muse.remove("jev")), "muse plugins remove jev --json");
  assert_eq!(words(&muse.update("jev")), "muse plugins update jev --json");
  assert_eq!(words(&muse.list_installed()), "muse plugins list --json");
  assert_eq!(
    words(&muse.list_available()),
    "muse plugins list --available --json"
  );
  assert_eq!(
    muse.inspect("jev").map(|argv| words(&argv)).as_deref(),
    Some("muse plugins inspect jev --json")
  );
  assert_eq!(
    muse.approve("jev").map(|argv| words(&argv)).as_deref(),
    Some("muse plugins approve jev --json")
  );
  assert_eq!(Adapter::Claude.inspect("jev"), None);
  assert_eq!(Adapter::Codex.approve("jev"), None);
  let Err(wrong) = muse.parse_list("[]") else {
    panic!("an array parsed as a muse listing");
  };
  assert!(wrong.starts_with("muse plugins list --json: "), "{wrong}");
}
