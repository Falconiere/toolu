//! A stateful `codex` stub for the tests that drive Codex's marketplace pin:
//! it stands in for Codex 0.164.0 only at the process boundary.

use crate::support::Sandbox;

/// Codex 0.164.0's refusal to add a source that differs from the registered one.
const REFUSAL: &str = "Error: marketplace 'toolu' is already added from a different source; remove it before adding this source";

/// A `codex` that keeps the `toolu` marketplace in `codex-marketplace`
/// (`none`, `git -` unpinned, `git <tag>`, or `local <dir>`) and answers its
/// marketplace commands the way Codex 0.164.0 does: a git add replaces a
/// local source and refuses a different git source. Flag files make it fail:
/// `codex-fail-pin` every pinned add, `codex-fail-pin-fresh` a pinned add with
/// no marketplace (after a remove), `codex-fail-restore` the unpinned add,
/// `codex-fail-remove` the remove, `codex-fail-list` the listing,
/// `codex-bad-list` the listing's shape, `codex-fail-plugin` a plugin add.
pub(crate) fn codex(sb: &Sandbox, state: &str) {
  let root = sb.root();
  let root = root.display();
  sb.write("codex-marketplace", &format!("{state}\n"));
  let body = format!(
    r#"echo "$*" >> {root}/codex.log
state=$(cat {root}/codex-marketplace)
for word in "$@"; do last=$word; done
case "$*" in
"plugin marketplace add Falconiere/toolu --ref "*)
  if [ -e {root}/codex-fail-pin ]; then echo "Error: cannot fetch $last" >&2; exit 1; fi
  if [ "$state" = none ] && [ -e {root}/codex-fail-pin-fresh ]; then echo "Error: cannot fetch $last" >&2; exit 1; fi
  case "$state" in
  none|local*) echo "git $last" > {root}/codex-marketplace ;;
  "git $last") echo "Marketplace \`toolu\` is already added." ;;
  *) echo "{REFUSAL}" >&2; exit 1 ;;
  esac ;;
"plugin marketplace add Falconiere/toolu")
  if [ -e {root}/codex-fail-restore ]; then echo "Error: network down" >&2; exit 1; fi
  if [ "$state" = none ]; then echo "git -" > {root}/codex-marketplace; fi ;;
"plugin marketplace remove toolu")
  if [ -e {root}/codex-fail-remove ]; then echo "Error: config is read-only" >&2; exit 1; fi
  echo none > {root}/codex-marketplace ;;
"plugin marketplace list --json")
  if [ -e {root}/codex-fail-list ]; then echo "error: unrecognized subcommand 'marketplace'" >&2; exit 2; fi
  if [ -e {root}/codex-bad-list ]; then echo '[]'; exit 0; fi
  case "$state" in
  none) echo '{{"marketplaces":[]}}' ;;
  local*) echo '{{"marketplaces":[{{"name":"toolu","marketplaceSource":{{"sourceType":"local","source":"/src/toolu"}}}}]}}' ;;
  *) echo '{{"marketplaces":[{{"name":"toolu","marketplaceSource":{{"sourceType":"git","source":"https://github.com/Falconiere/toolu.git"}}}}]}}' ;;
  esac ;;
"plugin list --json") cat {root}/codex-installed.json 2>/dev/null || echo '{{"installed":[]}}' ;;
"plugin list --available --json") cat {root}/codex-available.json 2>/dev/null || echo '{{"installed":[]}}' ;;
"plugin add "*)
  if [ -e {root}/codex-fail-plugin ]; then echo "Error: plugin '$last' not found" >&2; exit 1; fi ;;
esac"#
  );
  sb.stub("codex", &body);
}
