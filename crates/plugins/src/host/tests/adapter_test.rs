use super::{Adapter, Installed};
use crate::args::Scope;

fn words(argv: &[String]) -> String {
  argv.join(" ")
}

#[test]
fn claude_builds_marketplace_install_with_scope_remove_and_update() {
  let claude = Adapter::Claude;
  assert_eq!(words(&claude.add_marketplace()), "claude plugin marketplace add Falconiere/toolu");
  assert_eq!(
    words(&claude.install("rust-quality", Some(Scope::User))),
    "claude plugin install rust-quality@toolu --scope user"
  );
  assert_eq!(words(&claude.install("rust-quality", None)), "claude plugin install rust-quality@toolu");
  assert_eq!(words(&claude.remove("ast-grep")), "claude plugin uninstall ast-grep@toolu");
  assert_eq!(words(&claude.update("ast-grep")), "claude plugin update ast-grep@toolu");
  assert_eq!(words(&claude.list_installed()), "claude plugin list --json");
  assert_eq!(words(&claude.list_available()), "claude plugin list --available --json");
}

#[test]
fn codex_omits_scope_and_re_adds_to_update() {
  let codex = Adapter::Codex;
  assert_eq!(words(&codex.install("rust-quality", Some(Scope::User))), "codex plugin add rust-quality@toolu");
  assert_eq!(words(&codex.update("ast-grep")), "codex plugin add ast-grep@toolu");
  assert_eq!(words(&codex.remove("ast-grep")), "codex plugin remove ast-grep@toolu");
  assert_eq!(words(&codex.add_marketplace()), "codex plugin marketplace add Falconiere/toolu");
}

#[test]
fn listings_normalize_and_the_wrong_shape_is_an_error_naming_the_command() {
  let claude = Adapter::Claude.parse_list(r#"[{"id":"jev@toolu","version":"7.11.0","enabled":false}]"#);
  assert_eq!(
    claude,
    Ok(vec![Installed { name: "jev".to_owned(), version: "7.11.0".to_owned(), enabled: false }])
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
