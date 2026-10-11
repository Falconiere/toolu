use super::closure_problems;
use crate::pack_npm::PackedFile;

#[test]
fn a_missing_reference_is_a_problem() {
  let dir = tempfile::tempdir().expect("temp");
  std::fs::create_dir_all(dir.path().join("generated/skills/a")).expect("dir");
  std::fs::write(
    dir.path().join("package.json"),
    "{\"name\":\"@toolu/opencode\"}\n",
  )
  .expect("manifest");
  let skill = "bun \"$TOOLU_PLUGIN_ROOT/hooks/dist/ledger.js\" status\n";
  std::fs::write(dir.path().join("generated/skills/a/SKILL.md"), skill).expect("skill");
  let files = vec![PackedFile {
    path: "generated/skills/a/SKILL.md".to_owned(),
    mode: 0o644,
  }];
  let problems = closure_problems(dir.path(), &files, dir.path(), &[]).expect("closure");
  assert_eq!(
    problems,
    vec![
      "@toolu/opencode: generated/skills/a/SKILL.md references $TOOLU_PLUGIN_ROOT/hooks/dist/ledger.js, which the tarball does not contain"
        .to_owned()
    ]
  );
}
