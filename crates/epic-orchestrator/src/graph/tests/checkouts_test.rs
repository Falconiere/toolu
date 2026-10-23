use std::path::PathBuf;

use toolu_runtime::env::Env;

use std::collections::BTreeMap;

use super::{herdr_roots, origin_of, repo_roots, resolve_checkouts, resolve_clone_root};
use crate::paths::Paths;
use crate::support::repo_with_origin;

#[test]
fn origin_of_reads_a_real_checkout_and_refuses_a_non_repository() {
  let tmp = tempfile::tempdir().expect("temp");
  repo_with_origin(&tmp.path().join("ssh"), "git@github.com:Acme/API.git");
  repo_with_origin(&tmp.path().join("https"), "https://github.com/acme/web/");
  repo_with_origin(&tmp.path().join("other"), "https://gitlab.com/acme/web.git");
  assert_eq!(
    origin_of(&tmp.path().join("ssh")).as_deref(),
    Some("acme/api")
  );
  assert_eq!(
    origin_of(&tmp.path().join("https")).as_deref(),
    Some("acme/web")
  );
  assert_eq!(origin_of(&tmp.path().join("other")), None);
  assert_eq!(origin_of(tmp.path()), None);
}

#[test]
fn checkouts_come_from_known_roots_their_siblings_and_extra_parents() {
  let tmp = tempfile::tempdir().expect("temp");
  let known = tmp.path().join("work").join("api");
  repo_with_origin(&known, "https://github.com/Acme/api.git");
  repo_with_origin(
    &tmp.path().join("work").join("web"),
    "https://github.com/acme/web.git",
  );
  repo_with_origin(
    &tmp.path().join("extra").join("cli"),
    "https://github.com/acme/cli.git",
  );
  let repos = vec![
    "Acme/api".to_owned(),
    "acme/web".to_owned(),
    "acme/cli".to_owned(),
    "acme/gone".to_owned(),
  ];
  let found = resolve_checkouts(
    &repos,
    std::slice::from_ref(&known),
    &[tmp.path().join("extra")],
  );
  assert_eq!(
    found["Acme/api"].as_deref(),
    Some(known.display().to_string().as_str())
  );
  let exact = |rel: &[&str]| {
    let path = rel
      .iter()
      .fold(tmp.path().to_path_buf(), |dir, part| dir.join(part));
    Some(path.display().to_string())
  };
  assert_eq!(found["acme/web"], exact(&["work", "web"]));
  assert_eq!(found["acme/cli"], exact(&["extra", "cli"]));
  assert_eq!(found["acme/gone"], None);
}

#[test]
fn repo_roots_split_on_colons_and_herdr_roots_tolerate_no_herdr() {
  let env = Env::from_pairs([("EPIC_REPO_ROOTS", "/a::/b")]);
  assert_eq!(repo_roots(&env), [PathBuf::from("/a"), PathBuf::from("/b")]);
  assert_eq!(repo_roots(&Env::default()), Vec::<PathBuf>::new());
  let tmp = tempfile::tempdir().expect("temp");
  let absent = Env::from_pairs([
    ("HOME", tmp.path().display().to_string()),
    (
      "HERDR_SOCKET_PATH",
      tmp.path().join("none.sock").display().to_string(),
    ),
  ]);
  assert_eq!(
    herdr_roots(&Paths::at(tmp.path()), &absent),
    Vec::<PathBuf>::new()
  );
}

#[test]
fn clone_root_comes_from_known_roots_and_never_from_slash() {
  let none = BTreeMap::new();
  let root = |checkout: Option<&str>, roots: &[&str], herdr: &[&str]| {
    let paths = |list: &[&str]| list.iter().map(PathBuf::from).collect::<Vec<_>>();
    resolve_clone_root(checkout, &paths(roots), &paths(herdr), &none)
  };
  assert_eq!(
    root(Some("/a/b/repo"), &["/x"], &[]),
    Ok(PathBuf::from("/a/b"))
  );
  assert_eq!(root(None, &["/x"], &["/h/repo"]), Ok(PathBuf::from("/x")));
  assert_eq!(root(None, &[], &["/h/repo"]), Ok(PathBuf::from("/h")));
  assert_eq!(root(Some("/repo"), &["/"], &["/repo"]).ok(), None);
  let err = root(None, &[], &[]).expect_err("unresolved");
  assert!(err.starts_with("cannot resolve clone_root"), "{err}");
}
