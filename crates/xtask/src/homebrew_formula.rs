//! `cargo xtask homebrew-formula <tag> <sha256sums>`: the `Formula/toolu.rb` of
//! the `falconiere/tap` tap for a release, from its `SHA256SUMS` (#457).

use std::path::Path;

use regex::Regex;

use crate::options::Options;
use crate::{Verdict, output};

/// The release tag shape `install.sh` also accepts.
const TAG: &str = r"^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$";

/// Where the release assets live.
const REPOSITORY: &str = "https://github.com/Falconiere/toolu";

/// The four archives, as (`on_<os>`, `on_<cpu>`, asset name).
const TARGETS: [(&str, &str, &str); 4] = [
  ("macos", "arm", "toolu-darwin-arm64.tar.gz"),
  ("macos", "intel", "toolu-darwin-amd64.tar.gz"),
  ("linux", "arm", "toolu-linux-arm64.tar.gz"),
  ("linux", "intel", "toolu-linux-amd64.tar.gz"),
];

/// Print the formula for the tag and sums file in `options.files`.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let [tag, sums] = options.files.as_slice() else {
    return Err("homebrew-formula needs <tag> <sha256sums>".to_owned());
  };
  let tag = tag.to_string_lossy();
  let pattern = Regex::new(TAG).map_err(|err| format!("bad tag pattern: {err}"))?;
  if !pattern.is_match(&tag) {
    return Err(format!("tag {tag} is not vX.Y.Z"));
  }
  let text = read(sums)?;
  match formula(&tag, &text) {
    Ok(formula) => {
      output::say(&formula);
      Ok(Verdict::Clean)
    }
    Err(missing) => Ok(output::findings("homebrew-formula", &missing)),
  }
}

fn read(path: &Path) -> Result<String, String> {
  std::fs::read_to_string(path).map_err(|err| format!("cannot read {}: {err}", path.display()))
}

/// The hash `sums` gives `name`: a `<hex>  <name>` or `<hex> *<name>` line.
pub(crate) fn digest<'a>(sums: &'a str, name: &str) -> Option<&'a str> {
  sums.lines().find_map(|line| {
    let (hash, rest) = line.split_once(' ')?;
    let file = rest.strip_prefix(' ').unwrap_or(rest);
    let file = file.strip_prefix('*').unwrap_or(file);
    let hex = hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit());
    (hex && file == name).then_some(hash)
  })
}

/// The formula text, or one finding per archive the sums file lacks. Every
/// hash is resolved before any text is built, so a gap yields no formula.
pub(crate) fn formula(tag: &str, sums: &str) -> Result<String, Vec<String>> {
  let stanzas = TARGETS.map(|(_, cpu, name)| {
    digest(sums, name).map(|hash| {
      format!(
        "    on_{cpu} do\n      url \"{REPOSITORY}/releases/download/{tag}/{name}\"\n      \
         sha256 \"{hash}\"\n    end\n"
      )
    })
  });
  let [
    Some(darwin_arm),
    Some(darwin_intel),
    Some(linux_arm),
    Some(linux_intel),
  ] = stanzas
  else {
    return Err(
      TARGETS
        .iter()
        .zip(&stanzas)
        .filter(|(_, stanza)| stanza.is_none())
        .map(|((_, _, name), _)| format!("SHA256SUMS has no sha256 for {name}"))
        .collect(),
    );
  };
  let version = tag.strip_prefix('v').unwrap_or(tag);
  Ok(format!(
    "class Toolu < Formula\n  desc \"Code-quality rules for coding agents, as one native binary\"\n  \
     homepage \"{REPOSITORY}\"\n  version \"{version}\"\n  license \"MIT\"\n\n  \
     on_macos do\n{darwin_arm}{darwin_intel}  end\n\n  on_linux do\n{linux_arm}{linux_intel}  end\n\n  \
     def install\n    bin.install \"toolu\"\n  end\n\n  test do\n    \
     assert_match \"toolu #{{version}}\", shell_output(\"#{{bin}}/toolu --version\")\n  end\nend"
  ))
}

#[cfg(test)]
#[path = "tests/homebrew_formula_test.rs"]
mod tests;
