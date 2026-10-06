#!/usr/bin/env bash
# Install the native toolu binary from a GitHub release, verified.
#
#   curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash
#   curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash -s -- --version v1.2.3
#
# The release's SHA256SUMS must carry a minisign signature from the key below,
# and the archive must match it, before anything is written to the install
# directory. Needs bash, curl, tar and sha256sum (or shasum); unzip on macOS.
#
# Exit codes: 0 success, 1 network or verification failure, 2 usage or an
# unsupported platform.
set -euo pipefail

# The toolu release key. Its secret half signs SHA256SUMS in release-native.yml.
TOOLU_PUBLIC_KEY="RWQnIicG+ykLjLRK43obFWLb8l27IN4cpEJmb5jdEIsvb/3Xo+QY4Knc"

# minisign 0.11: its macOS zip is universal, its linux archive has both CPUs.
MINISIGN_URL="https://github.com/jedisct1/minisign/releases/download/0.11"
MINISIGN_LINUX_SHA256="f0a0954413df8531befed169e447a66da6868d79052ed7e892e50a4291af7ae0"
MINISIGN_DARWIN_SHA256="e7c410ae8b8960d7087392472b040bda9b2f307c76df0384ac37f9ad103fc893"

TAG_PATTERN='^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$'
REPO_PATTERN='^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$'
MAX_PAGES=5

REPO="${TOOLU_REPO:-Falconiere/toolu}"
API="${TOOLU_GITHUB_API:-https://api.github.com}"
DOWNLOAD_BASE="${TOOLU_DOWNLOAD_BASE:-https://github.com}"
INSTALL_DIR="/usr/local/bin"
VERSION=""
MODE="install"
WORK=""
STAGED=""

usage() {
  cat <<'EOF'
Install toolu, the native binary of the toolu plugins.

usage: install.sh [--version <tag>] [--install-dir <dir>] [--check | --uninstall]

  --version <tag>      install this release tag (vX.Y.Z) instead of the newest
  --install-dir <dir>  install into <dir> (default /usr/local/bin)
  --check              print what would be installed; download nothing
  --uninstall          remove <install-dir>/toolu
  -h, --help           show this help

environment:
  TOOLU_REPO           owner/name to install from (default Falconiere/toolu)

Roll back with --version <older-tag>.
EOF
}

die() {
  echo "toolu install: $2" >&2
  exit "$1"
}

cleanup() {
  if [ -n "$STAGED" ]; then rm -f "$STAGED"; fi
  if [ -n "$WORK" ]; then rm -rf "$WORK"; fi
}
trap cleanup EXIT

parse_args() {
  while [ "$#" -gt 0 ]; do
    case "$1" in
      -h | --help)
        usage
        exit 0
        ;;
      --check) set_mode check ;;
      --uninstall) set_mode uninstall ;;
      --version | --install-dir)
        [ "$#" -ge 2 ] || die 2 "$1 needs a value"
        if [ "$1" = --version ]; then VERSION="$2"; else INSTALL_DIR="$2"; fi
        shift
        ;;
      *) die 2 "unknown argument: $1 (see --help)" ;;
    esac
    shift
  done
  [ -n "$INSTALL_DIR" ] || die 2 "--install-dir needs a directory"
  if [ -n "$VERSION" ] && ! [[ "$VERSION" =~ $TAG_PATTERN ]]; then
    die 2 "--version must look like v1.2.3, got: $VERSION"
  fi
}

set_mode() {
  if [ "$MODE" != install ] && [ "$MODE" != "$1" ]; then
    die 2 "--check and --uninstall cannot be combined"
  fi
  MODE="$1"
}

check_repo() {
  if ! [[ "$REPO" =~ $REPO_PATTERN ]] || [[ "$REPO" == *..* ]]; then
    die 2 "TOOLU_REPO must be owner/name, got: $REPO"
  fi
}

detect_platform() {
  local kernel machine
  kernel="$(uname -s)"
  machine="$(uname -m)"
  case "$kernel" in
    Darwin) OS=darwin ;;
    Linux) OS=linux ;;
    *) die 2 "unsupported system $kernel/$machine; supported: Darwin or Linux on arm64 or amd64" ;;
  esac
  case "$machine" in
    x86_64 | amd64) ARCH=amd64 ;;
    aarch64 | arm64) ARCH=arm64 ;;
    *) die 2 "unsupported system $kernel/$machine; supported: Darwin or Linux on arm64 or amd64" ;;
  esac
  ASSET="toolu-$OS-$ARCH.tar.gz"
}

uninstall() {
  local target="$INSTALL_DIR/toolu"
  if [ ! -e "$target" ] && [ ! -L "$target" ]; then
    echo "toolu is not installed in $INSTALL_DIR"
    return
  fi
  rm -f "$target" 2>/dev/null || die 1 "cannot remove $target; re-run with sudo or pass --install-dir"
  [ ! -e "$target" ] || die 1 "cannot remove $target; re-run with sudo or pass --install-dir"
  echo "removed $target"
}

print_plan() {
  local label="${VERSION:-latest}"
  echo "os:          $OS"
  echo "arch:        $ARCH"
  echo "version:     $label"
  echo "install dir: $INSTALL_DIR"
  echo "download:    $(asset_url "${VERSION:-<latest>}" "$ASSET")"
}

asset_url() {
  echo "$DOWNLOAD_BASE/$REPO/releases/download/$1/$2"
}

fetch() {
  curl -fsSL --retry 2 -o "$2" "$1" || die 1 "download failed: $1"
}

# Print one line per release field of a GitHub releases page: `T <tag>`,
# `D` (draft), `P` (prerelease), `A <asset name>`, and `E` after each release.
release_fields() {
  awk '
    { text = text $0 "\n" }
    END {
      n = length(text); depth = 0
      for (i = 1; i <= n; i++) {
        c = substr(text, i, 1)
        if (c == "\"") {
          j = i + 1; s = ""
          while (j <= n) {
            d = substr(text, j, 1)
            if (d == "\\") { s = s d substr(text, j + 1, 1); j += 2; continue }
            if (d == "\"") break
            s = s d; j++
          }
          i = j
          if (kind[depth] == "{" && want[depth]) { key[depth] = s; want[depth] = 0 }
          else if (depth == 2 && key[2] == "tag_name") print "T " s
          else if (depth == 4 && key[2] == "assets" && key[4] == "name") print "A " s
        } else if (c == "{" || c == "[") {
          depth++; kind[depth] = c; want[depth] = (c == "{"); key[depth] = ""
        } else if (c == "}" || c == "]") {
          if (depth == 2 && c == "}") print "E"
          depth--
        } else if (c == ",") {
          if (kind[depth] == "{") want[depth] = 1
        } else if (c == "t" && substr(text, i, 4) == "true" && depth == 2) {
          if (key[2] == "draft") print "D"
          if (key[2] == "prerelease") print "P"
          i += 3
        }
      }
    }'
}

# Pick the newest stable release with this platform's archive, SHA256SUMS and
# its signature, walking at most MAX_PAGES pages of the releases API.
latest_tag() {
  local url="$API/repos/$REPO/releases?per_page=100" page=0 next
  local body="$WORK/releases.json" headers="$WORK/releases.headers"
  while [ -n "$url" ] && [ "$page" -lt "$MAX_PAGES" ]; do
    page=$((page + 1))
    curl -fsSL --retry 2 -H "Accept: application/vnd.github+json" -D "$headers" -o "$body" "$url" ||
      die 1 "cannot list releases: $url"
    local tag="" skip=0 have=0 field value
    while IFS=' ' read -r field value; do
      case "$field" in
        T) tag="$value" ;;
        D | P) skip=1 ;;
        A) case "$value" in "$ASSET" | SHA256SUMS | SHA256SUMS.minisig) have=$((have + 1)) ;; esac ;;
        E)
          if [ "$skip" = 0 ] && [ "$have" -ge 3 ] && [[ "$tag" =~ $TAG_PATTERN ]]; then
            echo "$tag"
            return
          fi
          tag="" skip=0 have=0
          ;;
      esac
    done < <(release_fields <"$body")
    next="$(tr -d '\r' <"$headers" | grep -i '^link:' | tr ',' '\n' | grep 'rel="next"' |
      sed -n 's/.*<\([^>]*\)>.*/\1/p' | head -n 1 || true)"
    case "$next" in "$API"/*) url="$next" ;; *) url="" ;; esac
  done
  die 1 "no stable release of $REPO has $ASSET, SHA256SUMS and SHA256SUMS.minisig"
}

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    die 1 "need sha256sum or shasum to verify the download"
  fi
}

# The verifier: TOOLU_MINISIGN, or the pinned minisign release checked by digest.
minisign_bin() {
  if [ -n "${TOOLU_MINISIGN:-}" ]; then
    [ -x "$TOOLU_MINISIGN" ] || die 1 "TOOLU_MINISIGN is not executable: $TOOLU_MINISIGN"
    echo "$TOOLU_MINISIGN"
    return
  fi
  local archive digest expected cpu
  if [ "$OS" = darwin ]; then
    archive="$WORK/minisign.zip"
    fetch "$MINISIGN_URL/minisign-0.11-macos.zip" "$archive"
    expected="$MINISIGN_DARWIN_SHA256"
  else
    archive="$WORK/minisign.tar.gz"
    fetch "$MINISIGN_URL/minisign-0.11-linux.tar.gz" "$archive"
    expected="$MINISIGN_LINUX_SHA256"
  fi
  digest="$(sha256 "$archive")"
  [ "$digest" = "$expected" ] || die 1 "minisign download has sha256 $digest, expected $expected"
  mkdir "$WORK/minisign"
  if [ "$OS" = darwin ]; then
    unzip -q -o "$archive" minisign -d "$WORK/minisign" || die 1 "cannot unpack minisign"
    echo "$WORK/minisign/minisign"
  else
    if [ "$ARCH" = arm64 ]; then cpu=aarch64; else cpu=x86_64; fi
    tar -xzf "$archive" -C "$WORK/minisign" "minisign-linux/$cpu/minisign" ||
      die 1 "cannot unpack minisign"
    echo "$WORK/minisign/minisign-linux/$cpu/minisign"
  fi
}

# Verify the signature, then the archive digest, then the archive layout.
verify() {
  local tag="$1" minisign="$2" pub expected actual listing
  pub="${TOOLU_MINISIGN_PUB:-}"
  if [ -z "$pub" ]; then
    pub="$WORK/toolu.pub"
    printf 'untrusted comment: toolu release key\n%s\n' "$TOOLU_PUBLIC_KEY" >"$pub"
  fi
  "$minisign" -q -Vm "$WORK/SHA256SUMS" -x "$WORK/SHA256SUMS.minisig" -p "$pub" >/dev/null 2>&1 ||
    die 1 "SHA256SUMS of $tag is not signed by the toolu release key; refusing to install"
  expected="$(awk -v name="$ASSET" '
    $1 ~ /^[0-9a-fA-F]+$/ && length($1) == 64 && ($2 == name || $2 == "*" name) && NF == 2 { print $1; exit }
  ' "$WORK/SHA256SUMS")"
  [ -n "$expected" ] || die 1 "SHA256SUMS of $tag has no line for $ASSET"
  actual="$(sha256 "$WORK/$ASSET")"
  [ "$actual" = "$expected" ] ||
    die 1 "checksum mismatch for $ASSET: expected $expected, got $actual"
  listing="$(tar -tzf "$WORK/$ASSET")" || die 1 "cannot read $ASSET"
  [ "$listing" = $'toolu\nLICENSE' ] || die 1 "$ASSET does not contain exactly toolu and LICENSE"
  tar -tvzf "$WORK/$ASSET" | awk 'substr($1, 1, 1) != "-" { bad = 1 } END { exit bad }' ||
    die 1 "$ASSET has a member that is not a regular file"
}

install_binary() {
  local tag="$1"
  local hint="re-run with sudo, or pass --install-dir (for example --install-dir ~/.local/bin)"
  if [ ! -d "$INSTALL_DIR" ]; then
    mkdir -p "$INSTALL_DIR" 2>/dev/null || die 1 "cannot create $INSTALL_DIR; $hint"
  fi
  [ -w "$INSTALL_DIR" ] || die 1 "$INSTALL_DIR is not writable; $hint"
  mkdir "$WORK/unpack"
  tar -xzf "$WORK/$ASSET" -C "$WORK/unpack" toolu || die 1 "cannot unpack $ASSET"
  STAGED="$INSTALL_DIR/.toolu-new.$$"
  cp "$WORK/unpack/toolu" "$STAGED" || die 1 "cannot write $STAGED; $hint"
  chmod 0755 "$STAGED"
  mv -f "$STAGED" "$INSTALL_DIR/toolu" || die 1 "cannot replace $INSTALL_DIR/toolu; $hint"
  STAGED=""
  echo "installed toolu $tag to $INSTALL_DIR/toolu"
  echo
  echo "next steps:"
  echo "  toolu plugins install"
  echo "  toolu doctor"
}

main() {
  parse_args "$@"
  check_repo
  if [ "$MODE" = uninstall ]; then
    uninstall
    return
  fi
  detect_platform
  if [ "$MODE" = check ]; then
    print_plan
    return
  fi
  if [ "$OS" = darwin ] && [ -z "${TOOLU_MINISIGN:-}" ] && ! command -v unzip >/dev/null 2>&1; then
    die 1 "unzip is required on macOS to unpack the minisign verifier"
  fi
  WORK="$(mktemp -d "${TMPDIR:-/tmp}/toolu-install.XXXXXX")"
  local tag="$VERSION" minisign
  if [ -z "$tag" ]; then tag="$(latest_tag)"; fi
  minisign="$(minisign_bin)"
  fetch "$(asset_url "$tag" "$ASSET")" "$WORK/$ASSET"
  fetch "$(asset_url "$tag" SHA256SUMS)" "$WORK/SHA256SUMS"
  fetch "$(asset_url "$tag" SHA256SUMS.minisig)" "$WORK/SHA256SUMS.minisig"
  verify "$tag" "$minisign"
  install_binary "$tag"
}

main "$@"
