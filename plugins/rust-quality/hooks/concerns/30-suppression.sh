
# Forbidden lint suppression: #[allow(...)] / #![allow(...)] / #[expect(...)]
# and the #[cfg_attr(..., allow(...))] / cfg_attr(..., expect(...)) back door.
# Known limitation: grep is line-based, so an attribute split across multiple
# lines (e.g. `#[cfg_attr(\n  test,\n  allow(...))]`) escapes detection. Rare;
# multi-line attribute parsing isn't worth a full tokenizer here.
#
# EXEMPTION (tests/ only): the file-level INNER header — `#![allow(...)]` and
# the `#![cfg_attr(...)]` form of it — is not flagged in a test file. The lints
# a test suppresses there (unwrap_used, expect_used, panic, float_cmp,
# too_many_lines) are ones test code violates on purpose: a test that must
# panic to fail is not a defect being hidden, and the toolu-conventions Rust
# template prescribes exactly this header. Only the INNER form earns it —
# a per-item `#[allow(...)]` inside a test file silences one real warning on
# one real item and stays banned, as everywhere else. Same shape as
# ts-quality's narrower @ts-expect-error exemption for .test/.spec files.
_rs_suppress_re='^[[:space:]]*#!?\[(allow|expect)\(|^[[:space:]]*#!?\[cfg_attr\([^]]*\b(allow|expect)\b'
_rs_suppress_hint=""
if [[ "$FILE_PATH" == */tests/* ]]; then
  _rs_suppress_re='^[[:space:]]*#\[(allow|expect)\(|^[[:space:]]*#\[cfg_attr\([^]]*\b(allow|expect)\b'
  _rs_suppress_hint=" In a test file only the file-level #![allow(...)] header is accepted."
fi
if grep -qE "$_rs_suppress_re" "$FILE_PATH" 2>/dev/null; then
  add_error "Forbidden lint suppression (#[allow]/#[expect]/cfg_attr allow) in $FILE_PATH — remove it and fix the underlying warning in code. For unsafe_code, override in Cargo.toml [lints.rust].${_rs_suppress_hint}"
fi

