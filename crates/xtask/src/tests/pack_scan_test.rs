use super::normalize;

#[test]
fn a_relative_import_drops_the_dot_segment() {
  assert_eq!(normalize("./src/a.ts"), "src/a.ts");
  assert_eq!(
    normalize("generated/skills/a/../b/SKILL.md"),
    "generated/skills/b/SKILL.md"
  );
}
