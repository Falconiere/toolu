#[test]
fn a_skill_without_frontmatter_is_a_finding() {
  let tmp = tempfile::tempdir().unwrap();
  let path = tmp.path().join("plugins/demo/skills/demo/SKILL.md");
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(&path, "# no frontmatter\n").unwrap();
  let err = super::check_skills(tmp.path()).unwrap_err();
  assert!(err.contains("missing opening frontmatter"), "{err}");
}
