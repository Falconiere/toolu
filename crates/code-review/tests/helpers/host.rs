//! A subprocess fixture for a host CLI, with its prompt captured on disk.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

pub(crate) fn host(dir: &Path, name: &str, stdout: &str) -> std::io::Result<()> {
  std::fs::write(dir.join("response.json"), stdout)?;
  let binary = dir.join(name);
  let quote = |path: &Path| format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"));
  let script = format!(
    "#!/bin/sh\n/bin/cat > {}\n/bin/cat {}\n",
    quote(&dir.join("prompt.txt")),
    quote(&dir.join("response.json"))
  );
  std::fs::write(&binary, script)?;
  std::fs::set_permissions(binary, std::fs::Permissions::from_mode(0o755))
}
