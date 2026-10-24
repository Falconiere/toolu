use super::{Prepared, run};
use crate::args::{Approval, Verb};
use crate::support::Sandbox;

fn checkout_dir(sb: &Sandbox) -> String {
  sb.path("home/.local/share/toolu/muse-marketplace")
    .display()
    .to_string()
}

fn head_tag(sb: &Sandbox) -> String {
  let dir = sb.path("home/.local/share/toolu/muse-marketplace");
  std::fs::read_to_string(dir.join(".agents/plugins/marketplace.json"))
    .expect("a file the test wrote")
}

#[test]
fn a_clean_home_clones_the_tag_and_registers_the_marketplace() {
  let sb = Sandbox::new();
  sb.muse("never");
  let origin = sb.origin(&["v9.9.8", "v9.9.9"]);
  let driver = sb.muse_driver("9.9.8", &origin, Approval::Report);
  assert_eq!(run(&driver, Verb::Install), Ok(Prepared::default()));
  assert_eq!(head_tag(&sb), r#"{"tag":"v9.9.8"}"#);
  assert_eq!(
    sb.log("muse"),
    [
      "plugins marketplace list --json".to_owned(),
      format!("plugins marketplace add toolu {} --json", checkout_dir(&sb)),
    ]
  );
}

#[test]
fn an_existing_checkout_moves_to_the_new_tag_and_refreshes_the_snapshot() {
  let sb = Sandbox::new();
  sb.muse("never");
  let origin = sb.origin(&["v9.9.8"]);
  assert_eq!(
    run(
      &sb.muse_driver("9.9.8", &origin, Approval::Report),
      Verb::Install
    ),
    Ok(Prepared::default())
  );
  sb.write(
    "origin/.agents/plugins/marketplace.json",
    r#"{"tag":"v9.9.9"}"#,
  );
  let repo = sb.path("origin").display().to_string();
  sb.git(&["-C", &repo, "commit", "--quiet", "--all", "-m", "v9.9.9"]);
  sb.git(&["-C", &repo, "tag", "v9.9.9"]);
  sb.capture("marketplace-list", "muse-marketplaces.json");
  assert_eq!(
    run(
      &sb.muse_driver("9.9.9", &origin, Approval::Report),
      Verb::Update
    ),
    Ok(Prepared::default())
  );
  assert_eq!(head_tag(&sb), r#"{"tag":"v9.9.9"}"#);
  assert_eq!(
    sb.log("muse").last().map(String::as_str),
    Some("plugins marketplace update toolu --json")
  );
}

#[test]
fn a_foreign_toolu_marketplace_stops_before_any_install() {
  let sb = Sandbox::new();
  sb.muse("never");
  let origin = sb.origin(&["v9.9.8"]);
  sb.capture("marketplace-list", "muse-marketplaces.json");
  let listing =
    std::fs::read_to_string(sb.path("muse-marketplaces.json")).expect("a file the test wrote");
  sb.write(
    "muse-marketplaces.json",
    &listing.replace(&checkout_dir(&sb), "/srv/elsewhere"),
  );
  let Err(fail) = run(
    &sb.muse_driver("9.9.8", &origin, Approval::Report),
    Verb::Install,
  ) else {
    panic!("a foreign marketplace was accepted");
  };
  assert!(
    fail.message.contains("/srv/elsewhere")
      && fail
        .message
        .contains("muse plugins marketplace remove toolu"),
    "{}",
    fail.message
  );
  assert_eq!(sb.log("muse"), ["plugins marketplace list --json"]);
}

#[test]
fn a_directory_that_is_not_a_toolu_checkout_is_left_alone() {
  let sb = Sandbox::new();
  sb.muse("never");
  let origin = sb.origin(&["v9.9.8"]);
  sb.write("home/.local/share/toolu/muse-marketplace/notes.txt", "mine");
  let Err(fail) = run(
    &sb.muse_driver("9.9.8", &origin, Approval::Report),
    Verb::Install,
  ) else {
    panic!("a foreign directory was used");
  };
  assert!(
    fail.message.contains("is not a toolu checkout"),
    "{}",
    fail.message
  );
  assert!(
    sb.path("home/.local/share/toolu/muse-marketplace/notes.txt")
      .is_file()
  );
  assert_eq!(sb.log("muse"), Vec::<String>::new());
}

#[test]
fn a_checkout_from_another_origin_is_refused() {
  let sb = Sandbox::new();
  sb.muse("never");
  let origin = sb.origin(&["v9.9.8"]);
  assert!(
    run(
      &sb.muse_driver("9.9.8", &origin, Approval::Report),
      Verb::Install
    )
    .is_ok()
  );
  let Err(fail) = run(
    &sb.muse_driver("9.9.8", "https://example.com/fork.git", Approval::Report),
    Verb::Install,
  ) else {
    panic!("another origin was accepted");
  };
  assert!(fail.message.contains("is cloned from"), "{}", fail.message);
}

#[test]
fn a_missing_tag_fails_before_muse_runs() {
  let sb = Sandbox::new();
  sb.muse("never");
  let origin = sb.origin(&["v9.9.8"]);
  let Err(fail) = run(
    &sb.muse_driver("0.0.0", &origin, Approval::Report),
    Verb::Install,
  ) else {
    panic!("a missing tag cloned");
  };
  assert!(fail.message.starts_with("git clone"), "{}", fail.message);
  assert!(!sb.path("home/.local/share/toolu/muse-marketplace").exists());
  assert_eq!(sb.log("muse"), Vec::<String>::new());
}

#[test]
fn a_failing_marketplace_command_stops_the_run() {
  let sb = Sandbox::new();
  sb.muse("\"plugins marketplace add\"*");
  let origin = sb.origin(&["v9.9.8"]);
  let Err(fail) = run(
    &sb.muse_driver("9.9.8", &origin, Approval::Report),
    Verb::Install,
  ) else {
    panic!("a failed marketplace add passed");
  };
  assert!(
    fail.message.contains("boom: plugins marketplace add"),
    "{}",
    fail.message
  );
  sb.write("muse-marketplaces.json", "[]");
  let Err(fail) = run(
    &sb.muse_driver("9.9.8", &origin, Approval::Report),
    Verb::Install,
  ) else {
    panic!("a malformed marketplace list passed");
  };
  assert!(
    fail.message.contains("\"marketplaces\""),
    "{}",
    fail.message
  );
}

#[test]
fn an_unreadable_checkout_path_is_an_error_not_a_missing_checkout() {
  let sb = Sandbox::new();
  sb.muse("never");
  let origin = sb.origin(&["v9.9.8"]);
  sb.write("data", "a regular file where the data directory should be");
  let mut driver = sb.muse_driver("9.9.8", &origin, Approval::Report);
  if let Some(checkout) = driver.checkout.as_mut() {
    checkout.dir = sb.path("data/toolu/muse-marketplace");
  }
  let Err(fail) = run(&driver, Verb::Install) else {
    panic!("an unreadable checkout path passed");
  };
  assert!(
    fail.message.starts_with(&format!(
      "cannot read {}: ",
      sb.path("data/toolu/muse-marketplace").display()
    )),
    "{}",
    fail.message
  );
  assert_eq!(sb.log("muse"), Vec::<String>::new());
  sb.write(
    "home/.local/share/toolu/muse-marketplace/.agents",
    "a file, not a directory",
  );
  let Err(fail) = run(
    &sb.muse_driver("9.9.8", &origin, Approval::Report),
    Verb::Install,
  ) else {
    panic!("an unreadable catalog path passed");
  };
  assert!(fail.message.starts_with("cannot read "), "{}", fail.message);
}
