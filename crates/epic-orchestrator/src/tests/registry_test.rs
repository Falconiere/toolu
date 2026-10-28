use super::*;
use crate::control::register;

#[test]
fn concurrent_registrations_keep_every_entry() {
  let tmp = tempfile::tempdir().unwrap();
  let paths = Paths::at(tmp.path());
  std::thread::scope(|scope| {
    for i in 0..12 {
      let paths = &paths;
      scope.spawn(move || register(paths, &format!("/epic-{i}")).unwrap());
    }
  });
  assert_eq!(read(&paths).unwrap()["epics"].as_array().unwrap().len(), 12);
}

#[test]
fn damaged_registry_and_busy_lock_fail_without_replacing_contents() {
  let tmp = tempfile::tempdir().unwrap();
  let paths = Paths::at(tmp.path());
  for body in ["", "null", "{}", "{", "{\"epics\":false}"] {
    std::fs::write(paths.registry(), body).unwrap();
    assert!(register(&paths, "/new").is_err());
    assert_eq!(std::fs::read_to_string(paths.registry()).unwrap(), body);
  }
  std::fs::write(paths.root.join("registry.lock"), "unknown owner").unwrap();
  assert!(update(&paths, || Ok(())).is_err());
}

#[test]
fn cleanup_and_registration_do_not_lose_new_entries() {
  let tmp = tempfile::tempdir().unwrap();
  let paths = Paths::at(tmp.path());
  register(&paths, paths.root.join("missing").to_str().unwrap()).unwrap();
  std::thread::scope(|scope| {
    scope.spawn(|| crate::cleanup::apply(&paths, None, false).unwrap());
    for i in 0..8 {
      let paths = &paths;
      scope.spawn(move || {
        let dir = paths.root.join(format!("active-{i}"));
        std::fs::create_dir_all(&dir).unwrap();
        register(paths, dir.to_str().unwrap()).unwrap();
      });
    }
  });
  assert_eq!(read(&paths).unwrap()["epics"].as_array().unwrap().len(), 8);
}
