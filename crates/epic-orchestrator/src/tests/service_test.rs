use std::path::Path;

use toolu_protocol::exit::Exit;
use toolu_runtime::env::Env;

use super::{install_unit, service};
use crate::paths::Paths;

#[test]
fn service_writes_the_unit_for_its_home() {
  let tmp = tempfile::tempdir().expect("temp");
  let resources = tmp.path().join("resources");
  let home = tmp.path().join("home");
  std::fs::create_dir_all(&resources).expect("resources");
  std::fs::create_dir_all(&home).expect("home");
  let env = Env::from_pairs([
    ("HOME", home.display().to_string()),
    ("TOOLU_RESOURCE_HOME", resources.display().to_string()),
    ("PATH", "/home/user/.local/bin:/usr/bin".to_owned()),
    ("HERDR_ENV", "/home/user/.config/herdr/env".to_owned()),
    ("HERDR_SOCKET_PATH", "/run/user/1000/herdr.sock".to_owned()),
    ("HERDR_BIN_PATH", "/home/user/.local/bin/herdr".to_owned()),
    ("TYPESAFE_API_KEY", "sentinel-secret".to_owned()),
  ]);
  let paths = Paths::from_env(&env);
  let outcome = service(&env);
  assert_eq!(outcome.exit, Exit::Success, "{outcome:?}");
  let body = std::fs::read_to_string(paths.service()).expect("unit");
  assert!(body.contains("WantedBy=default.target"));
  assert!(body.contains(&paths.root.display().to_string()));
  assert!(body.contains("Environment=\"PATH=/home/user/.local/bin:/usr/bin\""));
  assert!(body.contains("Environment=\"HERDR_ENV=/home/user/.config/herdr/env\""));
  assert!(body.contains("Environment=\"HERDR_SOCKET_PATH=/run/user/1000/herdr.sock\""));
  assert!(body.contains("Environment=\"HERDR_BIN_PATH=/home/user/.local/bin/herdr\""));
  assert!(body.contains("EnvironmentFile=-%h/.config/toolu/epic.env"));
  assert!(body.contains("Restart=on-failure"));
  assert!(body.contains("RestartSec=5s"));
  assert!(!body.contains("sentinel-secret"));
  let user = home.join(".config/systemd/user/toolu-epic.service");
  assert_eq!(std::fs::read_to_string(user).expect("user unit"), body);
}

#[test]
fn service_escapes_values_and_rejects_line_breaks_before_writing() {
  let tmp = tempfile::tempdir().expect("temp");
  let paths = Paths::at(tmp.path().join("resources with space"));
  let home = tmp.path().join("home");
  let exe = Path::new("/opt/toolu bin/toolu");
  let env = Env::from_pairs([("PATH", "/bin/quoted\"dir\\and%percent")]);
  install_unit(&paths, &home, exe, &env).expect("unit");
  let body = std::fs::read_to_string(paths.service()).expect("unit");
  assert!(body.contains("ExecStart=\"/opt/toolu bin/toolu\" epic engine"));
  assert!(body.contains("Environment=\"PATH=/bin/quoted\\\"dir\\\\and%%percent\""));
  assert!(body.contains("Environment=\"TOOLU_RESOURCE_HOME="));
  std::fs::remove_file(paths.service()).expect("remove unit");
  std::fs::remove_file(home.join(".config/systemd/user/toolu-epic.service"))
    .expect("remove user unit");
  let invalid = env.with("HERDR_ENV", "first\nsecond");
  let err = install_unit(&paths, &home, exe, &invalid).expect_err("invalid");
  assert!(err.contains("line break"));
  assert!(!paths.service().exists());
  assert!(
    !home
      .join(".config/systemd/user/toolu-epic.service")
      .exists()
  );
}

#[test]
fn service_install_write_failures_name_the_target() {
  let tmp = tempfile::tempdir().expect("temp");
  let env = Env::default();
  let home = tmp.path().join("home-file");
  std::fs::write(&home, "x").expect("home");
  let err =
    install_unit(&Paths::at(tmp.path()), &home, Path::new("/bin/toolu"), &env).expect_err("home");
  assert!(err.contains("could not write"));
  let file = tmp.path().join("not-a-dir");
  std::fs::write(&file, "x").expect("file");
  let err =
    install_unit(&Paths::at(&file), tmp.path(), Path::new("/bin/toolu"), &env).expect_err("root");
  assert!(err.contains("could not write"));
}
