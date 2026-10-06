#!/usr/bin/env bash
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"
target=x86_64-unknown-linux-musl
budget=4194304

cargo test --release --target "$target" -p toolu-http --test transport
cargo build --release --target "$target" -p toolu-cli
cli="$repo_root/target/$target/release/toolu"
file "$cli" | grep -Eq '(static-pie|statically) linked'
cli_size=$(stat -c %s "$cli")
test "$cli_size" -le "$budget"

probe_dir=$(mktemp -d)
trap 'rm -rf "$probe_dir"' EXIT
mkdir -p "$probe_dir/src"
cat > "$probe_dir/Cargo.toml" <<TOML
[package]
name = "toolu-http-size-probe"
version = "0.0.0"
edition = "2024"

[dependencies]
toolu-http = { path = "$repo_root/crates/core/http" }
toolu-runtime = { path = "$repo_root/crates/core/runtime" }

[profile.release]
lto = "fat"
codegen-units = 1
strip = true
panic = "unwind"
TOML
cat > "$probe_dir/src/main.rs" <<'RUST'
use toolu_http::{Auth, Client, Config};
use toolu_runtime::env::Env;

fn main() {
  let url = std::env::args().nth(1).unwrap_or_else(|| "https://api.example.test/".into());
  let client = Client::new(Config::default(), &Env::process()).expect("client");
  let body = client.get_bytes(&url, &Auth::None).expect("request");
  println!("{}", body.len());
}
RUST
cargo build --release --target "$target" --manifest-path "$probe_dir/Cargo.toml" --target-dir "$repo_root/target/http-probe"
probe="$repo_root/target/http-probe/$target/release/toolu-http-size-probe"
file "$probe" | grep -Eq '(static-pie|statically) linked'
probe_size=$(stat -c %s "$probe")
test "$probe_size" -le "$budget"
if cargo tree -p toolu-http -e normal --prefix none | grep -Ei '^(tokio|hyper)( |$)'; then
  printf 'unexpected async HTTP dependency\n' >&2
  exit 1
fi
printf 'static musl TLS tests passed; toolu=%s bytes, linked HTTP probe=%s bytes (budget=%s)\n' "$cli_size" "$probe_size" "$budget"
