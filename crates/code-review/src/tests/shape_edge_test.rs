use std::fs;

use super::git_io::GIT_OUTPUT_MAX_BYTES;
use super::tests::{commit, git, isolated_env, repo};
use super::{ShapeError, shape};

#[test]
fn shape_anchor_literal_bracket_path_does_not_include_another_file() {
  let dir = repo();
  fs::create_dir(dir.path().join("app")).expect("app");
  fs::write(dir.path().join("app/[slug].txt"), "LITERAL\n").expect("literal path");
  fs::write(dir.path().join("app/s.txt"), "OTHER\n").expect("other path");
  commit(dir.path());
  let shaped = shape(&isolated_env(), dir.path(), Some("main")).expect("shape");
  assert_eq!(shaped.coverage.reviewed, ["app/[slug].txt", "app/s.txt"]);
  assert_eq!(shaped.files.len(), 2);
  assert_eq!(shaped.files[0].lines[0].text, "LITERAL");
  assert_eq!(shaped.files[1].lines[0].text, "OTHER");
}

#[test]
fn shape_coverage_ignores_branch_attributes_and_runs_from_a_subdirectory() {
  let dir = repo();
  fs::write(dir.path().join(".gitattributes"), "* linguist-generated\n").expect("attributes");
  fs::create_dir(dir.path().join("src")).expect("src");
  fs::write(dir.path().join("src/lib.rs"), "fn f() {}\n").expect("source");
  commit(dir.path());
  let env = isolated_env();
  let top = shape(&env, dir.path(), Some("main")).expect("shape");
  assert_eq!(top.coverage.reviewed, [".gitattributes", "src/lib.rs"]);
  let sub = shape(&env, &dir.path().join("src"), Some("main")).expect("shape subdirectory");
  assert_eq!(sub.coverage, top.coverage);
}

#[test]
fn shape_anchor_rename_covers_both_gate_paths_when_renames_are_disabled() {
  let dir = repo();
  git(dir.path(), &["mv", "readme.txt", "moved.txt"]);
  fs::write(
    dir.path().join("moved.txt"),
    "one\nTWO\nthree\nfour\nfive\nsix\n",
  )
  .expect("rename edit");
  commit(dir.path());
  git(dir.path(), &["config", "diff.renames", "false"]);
  let shaped = shape(&isolated_env(), dir.path(), Some("main")).expect("shape");
  assert_eq!(shaped.gate_files, ["moved.txt", "readme.txt"]);
  assert_eq!(shaped.coverage.reviewed, shaped.gate_files);
  assert_eq!(shaped.files.len(), 1);
  assert_eq!(shaped.renames.len(), 1);
  assert_eq!(shaped.renames[0].old_path, "readme.txt");
  assert_eq!(shaped.renames[0].path, "moved.txt");
}

#[test]
fn shape_coverage_reviews_source_moved_into_build_directory() {
  let dir = repo();
  fs::create_dir(dir.path().join("build")).expect("build");
  git(dir.path(), &["mv", "readme.txt", "build/readme.txt"]);
  fs::write(
    dir.path().join("build/readme.txt"),
    "one\nTWO\nthree\nfour\nfive\nsix\n",
  )
  .expect("rename edit");
  commit(dir.path());
  let shaped = shape(&isolated_env(), dir.path(), Some("main")).expect("shape");
  assert_eq!(shaped.coverage.reviewed, ["build/readme.txt"]);
  assert_eq!(shaped.coverage.skipped.len(), 0);
  assert_eq!(shaped.renames.len(), 1);
}

#[test]
fn shape_coverage_ignores_branch_diff_attribute_and_branch_marker() {
  let dir = repo();
  fs::write(
    dir.path().join(".gitattributes"),
    "attr.rs linguist-generated\nreadme.txt -diff\n",
  )
  .expect("branch attributes");
  fs::write(
    dir.path().join("readme.txt"),
    "// @generated\none\ntwo\nthree\nfour\nfive\nsix\n",
  )
  .expect("branch marker");
  commit(dir.path());
  let shaped = shape(&isolated_env(), dir.path(), Some("main")).expect("shape");
  assert_eq!(shaped.coverage.reviewed, [".gitattributes", "readme.txt"]);
  assert_eq!(shaped.coverage.skipped.len(), 0);
}

#[test]
fn shape_coverage_reviews_source_renamed_onto_generated_attribute() {
  let dir = repo();
  git(dir.path(), &["mv", "readme.txt", "attr.rs"]);
  fs::write(
    dir.path().join("attr.rs"),
    "one\nTWO\nthree\nfour\nfive\nsix\n",
  )
  .expect("rename edit");
  commit(dir.path());
  let shaped = shape(&isolated_env(), dir.path(), Some("main")).expect("shape");
  assert_eq!(shaped.coverage.reviewed, ["attr.rs"]);
  assert_eq!(shaped.renames.len(), 1);
}

#[test]
fn shape_anchor_keeps_carriage_return_in_changed_text() {
  let dir = repo();
  fs::write(dir.path().join("crlf.txt"), b"foo\r\n").expect("CRLF file");
  commit(dir.path());
  let shaped = shape(&isolated_env(), dir.path(), Some("main")).expect("shape");
  assert_eq!(shaped.files[0].lines[0].text, "foo\r");
}

#[test]
fn shape_base_reports_absent_or_option_like_base() {
  let dir = repo();
  for base in ["absent", "-x"] {
    assert!(matches!(
      shape(&isolated_env(), dir.path(), Some(base)),
      Err(ShapeError::BaseMissing(found)) if found == base
    ));
  }
}

#[test]
fn shape_anchor_ignores_user_inter_hunk_context_and_blank_suppression() {
  let dir = repo();
  fs::write(dir.path().join("gap.txt"), "a\n\nb\n").expect("base with a blank line");
  commit(dir.path());
  git(dir.path(), &["branch", "-f", "main"]);
  fs::write(dir.path().join("gap.txt"), "A\n\nB\n").expect("edits around the blank line");
  commit(dir.path());
  git(dir.path(), &["config", "diff.interHunkContext", "3"]);
  git(dir.path(), &["config", "diff.suppressBlankEmpty", "true"]);
  let shaped = shape(&isolated_env(), dir.path(), Some("main")).expect("shape");
  assert_eq!(shaped.coverage.reviewed, ["gap.txt"]);
  assert_eq!(shaped.files[0].lines.len(), 4);
  assert!(!shaped.files[0].patch.contains("\n\n"));
}

#[test]
fn shape_coverage_leaves_binary_to_symlink_typechange_unreviewed() {
  let dir = repo();
  fs::write(dir.path().join("link"), [0u8, 1, 2, 3]).expect("binary");
  commit(dir.path());
  git(dir.path(), &["branch", "-f", "main"]);
  fs::remove_file(dir.path().join("link")).expect("remove binary");
  std::os::unix::fs::symlink("readme.txt", dir.path().join("link")).expect("symlink");
  commit(dir.path());
  let shaped = shape(&isolated_env(), dir.path(), Some("main")).expect("shape");
  assert_eq!(shaped.coverage.skipped.len(), 0);
  assert_eq!(shaped.coverage.unreviewed.len(), 1);
  assert_eq!(
    shaped.coverage.unreviewed[0].reason,
    "Git patch has more than one file section"
  );
}

#[test]
fn shape_reviews_small_edit_when_late_generated_match_is_huge() {
  let dir = repo();
  let tail = format!(
    "a\nb\nc\nd\ngenerated{}\n",
    "x".repeat(GIT_OUTPUT_MAX_BYTES)
  );
  fs::write(dir.path().join("large.rs"), format!("old\n{tail}")).expect("base blob");
  commit(dir.path());
  git(dir.path(), &["branch", "-f", "main"]);
  fs::write(dir.path().join("large.rs"), format!("new\n{tail}")).expect("changed blob");
  fs::write(dir.path().join("readme.txt"), "changed\n").expect("second changed blob");
  commit(dir.path());
  let shaped = shape(&isolated_env(), dir.path(), Some("main")).expect("shape");
  assert_eq!(shaped.coverage.reviewed, ["large.rs", "readme.txt"]);
  assert_eq!(shaped.coverage.unreviewed.len(), 0);
  assert_eq!(shaped.files[0].lines[1].text, "new");
}

#[test]
fn shape_coverage_leaves_text_turned_binary_unreviewed() {
  let dir = repo();
  fs::write(dir.path().join("blob.bin"), [0u8, 1, 2]).expect("binary");
  commit(dir.path());
  git(dir.path(), &["branch", "-f", "main"]);
  fs::write(dir.path().join("readme.txt"), b"// \0\none\n").expect("NUL in text");
  fs::write(dir.path().join("blob.bin"), [0u8, 1, 3]).expect("binary edit");
  commit(dir.path());
  let shaped = shape(&isolated_env(), dir.path(), Some("main")).expect("shape");
  let skipped: Vec<_> = shaped
    .coverage
    .skipped
    .iter()
    .map(|item| (item.path.as_str(), item.reason.as_str()))
    .collect();
  assert_eq!(skipped, [("blob.bin", "binary")]);
  let unreviewed: Vec<_> = shaped
    .coverage
    .unreviewed
    .iter()
    .map(|item| (item.path.as_str(), item.reason.as_str()))
    .collect();
  assert_eq!(
    unreviewed,
    [("readme.txt", "content switched between text and binary")]
  );
}
