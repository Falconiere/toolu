use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use crate::{Assets, Error};

/// The 16x16 PNG of `TypeSafe`'s Box example.
const PNG: &[u8] = b"\x89\x50\x4e\x47\x0d\x0a\x1a\x0a\x00\x00\x00\x0d\x49\x48\x44\x52\x00\x00\x00\x10\x00\x00\x00\x10\x08\x02\x00\x00\x00\x90\x91\x68\x36\x00\x00\x00\x1c\x49\x44\x41\x54\x78\x9c\x63\xf8\x4f\x22\x60\xa0\xa7\x06\x06\x06\xec\x68\x54\x03\x7d\x35\x10\x07\x86\x83\x06\x00\xc0\x7a\x7d\x9f\xa6\x2b\x6c\x54\x00\x00\x00\x00\x49\x45\x4e\x44\xae\x42\x60\x82";

fn refusal(result: Result<(), Error>) -> String {
  match result {
    Err(Error::InvalidQuestion(reason)) => reason,
    other => format!("{other:?}"),
  }
}

#[test]
fn a_png_and_a_jpeg_become_base64_assets_in_order() {
  let mut assets = Assets::default();
  assert!(assets.is_empty());
  assets.image("shot", PNG).unwrap();
  assets.image("photo", b"\xff\xd8\xff\xe0rest").unwrap();
  assert!(!assets.is_empty());
  assert_eq!(
    assets.value().to_text(false),
    format!(
      r#"{{"shot":{{"type":"image","format":"png","data_base64":"{}"}},"photo":{{"type":"image","format":"jpg","data_base64":"/9j/4HJlc3Q="}}}}"#,
      STANDARD.encode(PNG)
    )
  );
}

#[test]
fn a_bad_reference_or_file_is_refused_and_adds_nothing() {
  let png = PNG.to_vec();
  let mut assets = Assets::default();
  assets.image("shot", &png).unwrap();
  let before = assets.clone();
  let long = "r".repeat(1025);
  let mut big = png.clone();
  big.resize(16 * 1024 * 1024 + 1, 0);
  for (reference, bytes, reason) in [
    ("", png.as_slice(), "an image needs a reference"),
    (" \n", png.as_slice(), "an image needs a reference"),
    (
      long.as_str(),
      png.as_slice(),
      "an image reference is at most 1024 bytes",
    ),
    ("shot", png.as_slice(), "image reference shot is repeated"),
    ("gif", b"GIF89a".as_slice(), "image gif is not PNG or JPEG"),
    ("empty", b"".as_slice(), "image empty is not PNG or JPEG"),
    ("big", big.as_slice(), "image big is over 16 MiB"),
  ] {
    assert_eq!(refusal(assets.image(reference, bytes)), reason);
  }
  assert_eq!(assets, before);
  assets.image(&"r".repeat(1024), &png).unwrap();
}
