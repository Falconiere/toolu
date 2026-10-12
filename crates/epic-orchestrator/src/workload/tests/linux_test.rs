use super::{parse_stat, processes, record};

const STAT: &str = "4242 (my (odd) cmd) S 4241 4240 4240 0 -1 4194560 120 0 0 0 1 2 0 0 20 0 1 0 987654 1000 10 18446744073709551615";

#[test]
fn a_stat_line_parses_past_a_command_with_parentheses() {
  let found = parse_stat(4242, STAT).expect("stat").expect("record");
  assert_eq!((found.parent, found.group), (4241, 4240));
  assert_eq!(found.started, "linux:987654");
  assert!(found.cwd.is_none() && !found.cwd_unavailable);
  let zombie = STAT.replacen(") S ", ") Z ", 1);
  assert_eq!(parse_stat(4242, &zombie), Ok(None));
  for bad in ["", "4242 (cmd", "4242 (cmd) S x 1", "4242 (cmd) S 1 2 3"] {
    assert_eq!(
      parse_stat(4242, bad),
      Err("invalid /proc stat for process 4242".to_owned()),
      "{bad}"
    );
  }
}

#[test]
fn the_live_inventory_reads_this_users_processes() {
  assert_eq!(record(u32::MAX - 7), Ok(None));
  let inventory = processes();
  if std::env::consts::OS != "linux" {
    assert!(inventory.is_err());
    return;
  }
  let inventory = inventory.expect("inventory");
  let me = inventory
    .iter()
    .find(|found| found.pid == std::process::id())
    .expect("self");
  let cwd = std::env::current_dir().expect("cwd").display().to_string();
  assert_eq!(me.cwd.as_deref(), Some(cwd.as_str()));
  assert!(record(std::process::id()).expect("self").is_some());
}
