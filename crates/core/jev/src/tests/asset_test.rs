use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use crate::{Assets, Error};

/// The 16x16 PNG of `TypeSafe`'s Box example, as its API reference encodes it.
const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAIAAACQkWg2AAAAHElEQVR4nGP4TyJgoKcGBgbsaFQDfTUQB4aDBgDAen2fpitsVAAAAABJRU5ErkJggg==";

fn refusal(result: Result<(), Error>) -> String {
  match result {
    Err(Error::InvalidQuestion(reason)) => reason,
    other => format!("{other:?}"),
  }
}

#[test]
fn a_png_and_a_jpeg_become_base64_assets_in_order() {
  let png = STANDARD.decode(PNG).unwrap();
  let mut assets = Assets::default();
  assert!(assets.is_empty());
  assets.image("shot", &png).unwrap();
  assets.image("photo", b"\xff\xd8\xff\xe0rest").unwrap();
  assert!(!assets.is_empty());
  assert_eq!(
    assets.value().to_text(false),
    format!(
      r#"{{"shot":{{"type":"image","format":"png","data_base64":"{PNG}"}},"photo":{{"type":"image","format":"jpg","data_base64":"/9j/4HJlc3Q="}}}}"#
    )
  );
}

#[test]
fn a_bad_reference_or_file_is_refused_and_adds_nothing() {
  let png = STANDARD.decode(PNG).unwrap();
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
