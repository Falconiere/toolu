use super::{identities, parse_lsof, parse_ps, processes};

#[test]
fn ps_rows_parse_and_zombies_drop() {
  let output = "  101     1   101 Ss   Sat Oct 11 20:51:03 2026\n  102   101   101 Z+   Sat Oct 11 20:51:04 2026\ngarbage\n  103   101   101 S    Sat Oct 11 20:51:05 2026  \n";
  let rows = parse_ps(output);
  let pids: Vec<u32> = rows.iter().map(|row| row.pid).collect();
  assert_eq!(pids, [101, 103]);
  assert_eq!(rows[1].parent, 101);
  assert_eq!(rows[1].started, "mac:Sat Oct 11 20:51:05 2026");
}

#[test]
fn lsof_fields_map_pids_to_cwds() {
  let cwds = parse_lsof("p101\nfcwd\nn/Users/me/wt\np102\nfcwd\nn/tmp\npbad\nn/ignored\n");
  assert_eq!(cwds.len(), 2);
  assert_eq!(cwds.get(&101).map(String::as_str), Some("/Users/me/wt"));
  assert_eq!(cwds.get(&102).map(String::as_str), Some("/tmp"));
}

#[test]
fn the_live_macos_inventory_sees_this_process() {
  if std::env::consts::OS != "macos" {
    return;
  }
  let me = std::process::id();
  assert!(identities().expect("ps").iter().any(|row| row.pid == me));
  let cwd = std::fs::canonicalize(std::env::current_dir().expect("cwd")).expect("canonical");
  let listed = processes().expect("lsof");
  let mine = listed.iter().find(|row| row.pid == me).expect("self");
  assert_eq!(
    mine.cwd.as_deref(),
    Some(cwd.display().to_string().as_str())
  );
}
