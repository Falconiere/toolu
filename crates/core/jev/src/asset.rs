//! The images a request carries: each listed once under `assets`, keyed by the
//! reference text that stands for it in the state or in a question.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use toolu_runtime::json::ordered::Ordered;

use crate::Error;

/// The largest image file the endpoint accepts.
const MAX_BYTES: usize = 16 * 1024 * 1024;

/// The longest reference the endpoint accepts, in bytes.
const MAX_REFERENCE: usize = 1024;

/// How a PNG file starts.
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";

/// How a JPEG file starts.
const JPEG: &[u8] = b"\xff\xd8\xff";

/// Images by reference, in the order they were added.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Assets(Vec<(String, Ordered)>);

impl Assets {
  /// Add the PNG or JPEG file `bytes` under `reference`. The endpoint puts the
  /// image wherever that text appears in the state, instructions or criteria.
  ///
  /// # Errors
  /// `InvalidQuestion`, before any request, for a blank, repeated or over-long
  /// reference, and for a file that is not PNG or JPEG or is over 16 MiB.
  pub fn image(&mut self, reference: &str, bytes: &[u8]) -> Result<(), Error> {
    let refuse = |reason: String| Err(Error::InvalidQuestion(reason));
    if reference.trim().is_empty() {
      return refuse("an image needs a reference".to_owned());
    }
    if reference.len() > MAX_REFERENCE {
      return refuse("an image reference is at most 1024 bytes".to_owned());
    }
    if self.0.iter().any(|(key, _)| key == reference) {
      return refuse(format!("image reference {reference} is repeated"));
    }
    let format = if bytes.starts_with(PNG) {
      "png"
    } else if bytes.starts_with(JPEG) {
      "jpg"
    } else {
      return refuse(format!("image {reference} is not PNG or JPEG"));
    };
    if bytes.len() > MAX_BYTES {
      return refuse(format!("image {reference} is over 16 MiB"));
    }
    // shortcut: base64 in JSON inflates images by a third against the 32 MiB body cap, upgrade to multipart when one request needs more than ~24 MiB of images
    let text = |value: &str| Ordered::String(value.to_owned());
    let asset = Ordered::Object(vec![
      ("type".to_owned(), text("image")),
      ("format".to_owned(), text(format)),
      ("data_base64".to_owned(), text(&STANDARD.encode(bytes))),
    ]);
    self.0.push((reference.to_owned(), asset));
    Ok(())
  }

  /// Whether the request carries no image.
  pub(crate) fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  /// The JSON object sent as `assets`.
  pub(crate) fn value(&self) -> Ordered {
    Ordered::Object(self.0.clone())
  }
}

#[cfg(test)]
#[path = "tests/asset_test.rs"]
mod tests;
