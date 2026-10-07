use super::matches;

#[test]
fn bash_patterns_cross_directories_and_support_extglob() {
  assert!(matches("hooks/**/*.sh", "hooks/lib/detect.sh"));
  assert!(matches("@(.env|.env.*|*secrets*)", ".env.local"));
  assert!(!matches("@(.env|.env.*|*secrets*)", "apps/api/.env.local"));
  assert!(matches(".en[v]", ".env"));
  assert!(matches("src/?.rs", "src/a.rs"));
  assert!(!matches("src/?.rs", "src/ab.rs"));
}
