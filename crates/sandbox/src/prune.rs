//! `prune`: the clones `run --clone` keeps on the sandbox's own disk, one per
//! checkout, for checkouts that no longer exist on this machine. Each clone's
//! `<slug>.src` names its checkout; a clone without one (made before `.src`
//! files) is reported, never removed. The VM is not booted to prune.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use toolu_protocol::exit::Exit;

use crate::context::Context;
use crate::step::{Failure, Report};

/// Prints one `<slug>\t<source>` line per clone under `$1/work` (an empty source
/// when the clone has no `.src` file); nothing when there is no `work` directory.
pub(crate) const LIST: &str = r#"cd "$1/work" 2>/dev/null || exit 0
for src in *.src; do
  [ -f "$src" ] || continue
  printf '%s\t%s\n' "${src%.src}" "$(cat "$src")"
done
for dir in */; do
  d=${dir%/}
  [ -d "$d" ] && [ ! -f "$d.src" ] && printf '%s\t\n' "$d"
done
exit 0
"#;

/// One clone on the sandbox disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Clone {
  /// The directory name under `work/`.
  pub(crate) slug: String,
  /// The checkout it was made from, when recorded.
  pub(crate) source: Option<PathBuf>,
}

/// A clone kept, with why.
pub(crate) type Kept = (Clone, &'static str);

/// The argv that lists the clones under `cache`.
pub(crate) fn list_argv(cache: &Path) -> Vec<String> {
  vec![
    "sh".to_owned(),
    "-c".to_owned(),
    LIST.to_owned(),
    "toolu-sandbox-prune".to_owned(),
    cache.to_string_lossy().into_owned(),
  ]
}

/// The clones [`LIST`] printed.
pub(crate) fn parse(text: &str) -> Vec<Clone> {
  text
    .lines()
    .filter_map(|line| {
      let (slug, source) = line.split_once('\t')?;
      (!slug.is_empty()).then(|| Clone {
        slug: slug.to_owned(),
        source: (!source.is_empty()).then(|| PathBuf::from(source)),
      })
    })
    .collect()
}

/// Split `clones` into those to remove (source gone) and those to keep, with the reason.
pub(crate) fn decide(
  clones: Vec<Clone>,
  exists: impl Fn(&Path) -> bool,
) -> (Vec<Clone>, Vec<Kept>) {
  let (mut remove, mut keep) = (Vec::new(), Vec::new());
  for clone in clones {
    match clone.source.as_deref() {
      None => keep.push((clone, "unknown origin: made before toolu recorded it")),
      Some(source) if exists(source) => keep.push((clone, "its checkout exists")),
      Some(_) => remove.push(clone),
    }
  }
  (remove, keep)
}

/// The argv that removes the clones `slugs` under `cache`, with their locks,
/// sources and cargo target directories.
pub(crate) fn remove_argv(cache: &Path, slugs: &[String]) -> Vec<String> {
  let mut argv = vec!["rm".to_owned(), "-rf".to_owned(), "--".to_owned()];
  for slug in slugs {
    // The `.src` goes last: a removal cut short keeps its origin known.
    for path in [
      cache.join("work").join(slug),
      cache.join("work").join(format!("{slug}.lock")),
      cache.join("target").join(slug),
      cache.join("work").join(format!("{slug}.src")),
    ] {
      argv.push(path.to_string_lossy().into_owned());
    }
  }
  argv
}

/// List, decide and (with `apply`) remove, running each argv through `run`.
///
/// # Errors
/// A listing or removal that fails.
pub(crate) fn prune_with(
  run: impl Fn(&[String]) -> Result<String, Failure>,
  cache: &Path,
  apply: bool,
) -> Result<(Vec<Clone>, Vec<Kept>), Failure> {
  let clones = parse(&run(&list_argv(cache))?);
  let (remove, keep) = decide(clones, Path::exists);
  if apply && !remove.is_empty() {
    let slugs: Vec<String> = remove.iter().map(|clone| clone.slug.clone()).collect();
    run(&remove_argv(cache, &slugs))?;
  }
  Ok((remove, keep))
}

fn row(clone: &Clone) -> Value {
  json!({"slug": clone.slug, "source": clone.source})
}

/// The prune report: `state` is the sandbox's, `removed` is "would remove" without `apply`.
pub(crate) fn report(
  context: &Context,
  state: &str,
  apply: bool,
  (removed, kept): (&[Clone], &[Kept]),
) -> Report {
  let verb = if apply { "removed" } else { "would remove" };
  let mut lines = vec![format!(
    "sandbox: {state}; {} {} orphan clone(s), {} kept",
    verb,
    removed.len(),
    kept.len()
  )];
  lines.extend(
    removed
      .iter()
      .map(|clone| format!("  {verb} {}", clone.slug)),
  );
  lines.extend(
    kept
      .iter()
      .map(|(clone, why)| format!("  kept {}: {why}", clone.slug)),
  );
  Report {
    exit: Exit::Success,
    doc: json!({
      "namespace": "sandbox",
      "verb": "prune",
      "backend": context.settings.backend.name(),
      "state": state,
      "apply": apply,
      "removed": removed.iter().map(row).collect::<Vec<_>>(),
      "kept": kept
        .iter()
        .map(|(clone, why)| json!({"slug": clone.slug, "source": clone.source, "reason": why}))
        .collect::<Vec<_>>(),
    }),
    text: lines.join("\n"),
  }
}

#[cfg(test)]
#[path = "tests/prune_test.rs"]
mod tests;
