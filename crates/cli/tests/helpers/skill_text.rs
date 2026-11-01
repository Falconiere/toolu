//! Reading a shipped `SKILL.md`: the body after its front matter and one
//! `## ` section.

/// The skill after its front matter.
pub(crate) fn body(text: &str) -> Result<&str, String> {
  text
    .splitn(3, "---\n")
    .nth(2)
    .ok_or_else(|| "no front matter".to_owned())
}

/// The text of the `## <title>` section, up to the next `## `.
pub(crate) fn section<'a>(text: &'a str, title: &str) -> Result<&'a str, String> {
  let (_, rest) = text
    .split_once(&format!("\n## {title}"))
    .ok_or_else(|| format!("no section {title}"))?;
  Ok(rest.split("\n## ").next().unwrap_or(rest))
}
