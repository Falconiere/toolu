use std::time::{Duration, Instant};

use toolu_runtime::process::{Spec, detach};

use super::{group_alive, process_start, stop_group, table};

const OLD: &str = "Thu Jan  1 00:00:00 1970";

fn moved_children(parent: u32) -> Vec<u32> {
  let started = Instant::now();
  loop {
    let moved: Vec<u32> = table()
      .unwrap()
      .iter()
      .filter(|row| row.parent == parent && row.group != parent)
      .map(|row| row.pid)
      .collect();
    if !moved.is_empty() || started.elapsed() > Duration::from_secs(10) {
      return moved;
    }
    std::thread::sleep(Duration::from_millis(100));
  }
}

#[test]
fn stop_group_ends_the_group_and_a_descendant_that_left_it() {
  let script = "sleep 30 & perl -e 'setpgrp(0, 0); sleep 30' & sleep 30";
  let pid = detach(&Spec::new(["sh", "-c", script])).unwrap();
  let start = process_start(pid);
  assert_ne!(start, "");
  let moved = moved_children(pid);
  assert_eq!(moved.len(), 1);
  assert!(group_alive(pid, &start).unwrap());
  assert!(
    !group_alive(pid, OLD).unwrap(),
    "a different start is a reused pid"
  );
  assert!(stop_group(pid, OLD).unwrap());
  assert!(group_alive(pid, &start).unwrap());
  assert!(stop_group(pid, &start).unwrap());
  assert!(!group_alive(pid, &start).unwrap());
  let live: Vec<u32> = table()
    .unwrap()
    .iter()
    .map(|row| row.pid)
    .filter(|pid| moved.contains(pid))
    .collect();
  assert_eq!(live, Vec::<u32>::new());
}

#[test]
fn no_process_is_neither_alive_nor_started() {
  assert!(!group_alive(0, "").unwrap());
  assert!(!group_alive(1, "").unwrap());
  assert_eq!(process_start(u32::MAX / 2), "");
  assert!(stop_group(u32::MAX / 2, "").unwrap());
}
