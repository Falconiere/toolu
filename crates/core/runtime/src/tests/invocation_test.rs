use super::{args, current_dir, current_exe};

#[test]
fn the_test_binary_knows_its_canonical_path() {
  let exe = current_exe().unwrap();
  assert!(exe.is_absolute());
  assert_eq!(std::fs::canonicalize(&exe).unwrap(), exe);
}

#[test]
fn argv_is_every_word_after_the_program_name() {
  let words: Vec<String> = std::env::args().skip(1).collect();
  assert_eq!(args(), words);
}

#[test]
fn the_working_directory_is_the_process_one() {
  assert_eq!(current_dir().unwrap(), std::env::current_dir().unwrap());
}
