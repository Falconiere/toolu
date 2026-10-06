use std::path::Path;

use super::{Walk, each_line};

/// The lines of the file at `path` and how the walk ended.
fn lines_of(path: &Path) -> (Vec<Vec<u8>>, Walk) {
  let mut lines = Vec::new();
  let walk = each_line(path, |line| {
    lines.push(line.to_vec());
    false
  });
  (lines, walk)
}

/// The lines of a file holding `body`.
fn lines_in(body: &[u8]) -> Vec<Vec<u8>> {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("f");
  std::fs::write(&file, body).unwrap();
  let (lines, walk) = lines_of(&file);
  assert_eq!(walk, Walk::Done);
  lines
}

#[test]
fn records_split_on_newline_like_awk() {
  assert_eq!(lines_in(b""), Vec::<Vec<u8>>::new());
  assert_eq!(lines_in(b"\n"), [b"".to_vec()]);
  assert_eq!(lines_in(b"a\n"), [b"a".to_vec()]);
  assert_eq!(
    lines_in(b"a\n\nb"),
    [b"a".to_vec(), b"".to_vec(), b"b".to_vec()]
  );
  assert_eq!(lines_in(b"\n\n\n").len(), 3);
}

#[test]
fn lines_are_bytes_and_a_carriage_return_is_content() {
  assert_eq!(
    lines_in(b"\xe9\xff\0x\r\nz\r"),
    [b"\xe9\xff\0x\r".to_vec(), b"z\r".to_vec()]
  );
}

#[test]
fn lines_spanning_chunks_arrive_whole() {
  let long = vec![b'y'; 200_000];
  let mut body = vec![b'x'; 65_535];
  body.push(b'\n');
  body.extend(&long);
  body.extend(b"\nshort\n");
  body.extend(vec![b'z'; 65_536]);
  let lines = lines_in(&body);
  let sizes: Vec<usize> = lines.iter().map(Vec::len).collect();
  assert_eq!(sizes, [65_535, 200_000, 5, 65_536]);
  assert_eq!(lines.get(1), Some(&long));
}

#[test]
fn the_visitor_can_stop_the_walk() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("f");
  std::fs::write(&file, "a\nb\nc\n").unwrap();
  let mut seen = Vec::new();
  let walk = each_line(&file, |line| {
    seen.push(line.to_vec());
    line == b"b"
  });
  assert_eq!(walk, Walk::Stopped);
  assert_eq!(seen, [b"a".to_vec(), b"b".to_vec()]);
}

#[test]
fn a_missing_file_is_unreadable_and_a_directory_reads_as_empty() {
  let dir = tempfile::tempdir().unwrap();
  assert_eq!(
    lines_of(&dir.path().join("missing")),
    (vec![], Walk::Unreadable)
  );
  assert_eq!(lines_of(dir.path()), (vec![], Walk::Done));
}

#[test]
fn a_file_that_opens_but_fails_to_read_is_unreadable() {
  // On Linux, reading a process's memory from offset 0 fails with EIO after a
  // successful open; elsewhere the path does not open. Either way nothing is read.
  let mem = Path::new("/proc/self/mem");
  assert_eq!(lines_of(mem), (vec![], Walk::Unreadable));
}
