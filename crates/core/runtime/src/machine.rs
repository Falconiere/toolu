//! The machine the binary runs on: its short host name.

/// This machine's host name up to its first `.`, or `None` when the system
/// call fails or the name is empty or not UTF-8.
pub fn short_hostname() -> Option<String> {
  let name = nix::unistd::gethostname().ok()?;
  short(name.to_str()?)
}

/// The current Unix user's numeric ID, for host-owned temporary paths.
pub fn user_id() -> u32 {
  nix::unistd::getuid().as_raw()
}

/// `name` up to its first `.`, or `None` when that part is empty.
fn short(name: &str) -> Option<String> {
  name
    .split('.')
    .next()
    .filter(|part| !part.is_empty())
    .map(str::to_owned)
}

#[cfg(test)]
#[path = "tests/machine_test.rs"]
mod tests;
