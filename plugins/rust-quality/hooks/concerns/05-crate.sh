# shellcheck shell=bash
# --- Crate gate (runs after lib/rust-rules.sh is concatenated in) ---
# Fire when the edited .rs file belongs to a Cargo crate, by EITHER signal:
#   1. detect_rust — a Cargo.toml at the resolved project root (the classic case)
#   2. nearest_cargo_toml — an enclosing Cargo.toml ABOVE the edited file
#      (the nested-workspace case: e.g. editing
#      packages/backend/crates/api/src/x.rs in a repo with no root Cargo.toml
#      still has crates/api/Cargo.toml above it). AC-15.
# No enclosing crate at all -> no-op (a stray .rs outside any crate is not ours).
# FILE_PATH is already resolved-and-.rs-guarded by 00-preamble.sh.
if [ "$(detect_rust)" != "rust" ] && [ -z "$(nearest_cargo_toml "$FILE_PATH")" ]; then
  exit 0
fi
