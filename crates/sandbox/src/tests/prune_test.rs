use std::path::{Path, PathBuf};

use toolu_runtime::process::{Spec, run};

use super::{Clone, decide, list_argv, parse, prune_with, remove_argv};
use crate::step::Failure;

fn sh(argv: &[String]) -> Result<String, Failure> {
  let output = run(&Spec::new(argv.iter().cloned()))
    .map_err(|err| Failure::new(toolu_protocol::exit::Exit::Failure, format!("{err:?}")))?;
  if output.exit_code == 0 {
    Ok(output.stdout)
  } else {
    Err(Failure::new(
      toolu_protocol::exit::Exit::Failure,
      output.stderr,
    ))
  }
}

#[test]
fn listing_and_removal_argv_name_every_path_as_an_argument() {
  let list = list_argv(Path::new("/var/cache/toolu-sandbox"));
  assert_eq!(&list[..2], ["sh", "-c"]);
  assert_eq!(list[4], "/var/cache/toolu-sandbox");
  let remove = remove_argv(Path::new("/c"), &["a-1".to_owned()]);
  assert_eq!(
    remove,
    [
      "rm",
      "-rf",
      "--",
      "/c/work/a-1",
      "/c/work/a-1.lock",
      "/c/target/a-1",
      "/c/work/a-1.src"
    ]
  );
}

#[test]
fn clones_without_a_source_or_with_a_live_checkout_stay() {
  let clones = parse("a-1\t/gone\nb-2\t/here\nc-3\t\n\n");
  assert_eq!(clones.len(), 3);
  assert_eq!(clones[2].source, None);
  let (remove, keep) = decide(clones, |path| path == Path::new("/here"));
  assert_eq!(
    remove,
    [Clone {
      slug: "a-1".into(),
      source: Some(PathBuf::from("/gone"))
    }]
  );
  assert_eq!(keep[0].1, "its checkout exists");
  assert_eq!(keep[1].1, "unknown origin: made before toolu recorded it");
}

#[test]
fn a_real_cache_loses_only_the_orphan_clone_when_applied() {
  let dir = tempfile::tempdir().unwrap();
  let cache = dir.path().join("cache");
  let live = dir.path().join("live");
  std::fs::create_dir_all(&live).unwrap();
  for slug in ["orphan-1", "live-2", "old-3"] {
    std::fs::create_dir_all(cache.join("work").join(slug)).unwrap();
    std::fs::create_dir_all(cache.join("target").join(slug)).unwrap();
  }
  std::fs::write(
    cache.join("work/orphan-1.src"),
    format!("{}\n", dir.path().join("gone").display()),
  )
  .unwrap();
  std::fs::write(cache.join("work/orphan-1.lock"), "").unwrap();
  std::fs::write(cache.join("work/live-2.src"), live.display().to_string()).unwrap();

  let (remove, keep) = prune_with(sh, &cache, false).unwrap();
  assert_eq!(remove.len(), 1);
  assert_eq!(keep.len(), 2);
  assert!(
    cache.join("work/orphan-1").exists(),
    "a report removes nothing"
  );

  prune_with(sh, &cache, true).unwrap();
  assert!(!cache.join("work/orphan-1").exists());
  assert!(!cache.join("work/orphan-1.src").exists());
  assert!(!cache.join("work/orphan-1.lock").exists());
  assert!(!cache.join("target/orphan-1").exists());
  assert!(cache.join("work/live-2").exists());
  assert!(cache.join("work/old-3").exists());
  let (remove, _) = prune_with(sh, &cache, true).unwrap();
  assert_eq!(remove, []);
  let (none, kept) = prune_with(sh, &dir.path().join("none"), true).unwrap();
  assert_eq!((none.len(), kept.len()), (0, 0));
}
