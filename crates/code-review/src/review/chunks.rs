//! One structured review call per shaped chunk. Every call shares the prefix
//! messages, so providers can cache them. A chunk whose prompt does not fit, or
//! whose call fails, leaves its paths unreviewed; a deadline expiry is typed and
//! never retried at a lower level.

use std::time::{Duration, Instant};

use serde_json::Value;
use toolu_openrouter::{Client, Message};

use super::calls::{Ledger, Stage};
use super::mask::Mask;
use super::prompt::{self, Prefix};
use super::source::Package;
use super::{Input, RunError};
use crate::budget::Effort;
use crate::contract::Omitted;
use crate::model::{self, ModelFinding};
use crate::shape::ShapedFile;

/// How each review call is made.
pub(crate) struct Call<'a> {
  pub(crate) client: &'a Client,
  pub(crate) schema: &'a Value,
  pub(crate) level: Effort,
  pub(crate) deadline: Duration,
}

/// The review calls' combined result.
#[derive(Default)]
pub(crate) struct Reviews {
  pub(crate) findings: Vec<ModelFinding>,
  pub(crate) unreviewed: Vec<Omitted>,
  pub(crate) notes: Vec<String>,
  failed: usize,
  last_error: Option<String>,
  only_deadlines: bool,
}

impl Reviews {
  /// The run's error when every one of `chunks` failed, with what they cost.
  pub(crate) fn error(&self, chunks: usize, ledger: &Ledger<'_>, model: &str) -> Option<RunError> {
    if chunks == 0 || self.failed < chunks {
      return None;
    }
    let usage = ledger.usage(Some(model.to_owned()));
    Some(match &self.last_error {
      Some(message) if !self.only_deadlines => RunError::Outage {
        message: format!("every review call failed: {message}"),
        usage,
      },
      Some(_) | None => RunError::Deadline { usage },
    })
  }

  fn fail(&mut self, paths: &[&str], reason: &str, deadline: bool) {
    self.only_deadlines = (self.failed == 0 || self.only_deadlines) && deadline;
    self.failed += 1;
    self.last_error = Some(reason.to_owned());
    self.unreviewed.extend(paths.iter().map(|path| Omitted {
      path: (*path).to_owned(),
      reason: reason.to_owned(),
    }));
  }
}

/// Review every chunk of `input` in order.
pub(crate) fn review(
  input: &Input<'_>,
  packages: &[Package],
  mask: &Mask,
  call: &Call<'_>,
  ledger: &mut Ledger<'_>,
) -> Reviews {
  let mut reviews = Reviews::default();
  let (prefix, note) = prompt::prefix(input.rules, mask);
  reviews.notes.extend(note);
  for chunk in &input.shaped.chunks {
    let files: Vec<_> = chunk
      .files
      .iter()
      .filter_map(|index| Some((input.shaped.files.get(*index)?, packages.get(*index)?)))
      .collect();
    let paths: Vec<&str> = files.iter().map(|(file, _)| file.path.as_str()).collect();
    let Some(last) = last_message(input, &files, &paths, mask, (&prefix, &mut reviews)) else {
      reviews.fail(&paths, "prompt over budget", false);
      continue;
    };
    let messages = [
      message("system", &prefix.system),
      message("user", &prefix.conventions),
      message("user", &last),
    ];
    send(&mut reviews, &paths, &messages, call, ledger);
  }
  reviews
}

/// The chunk's message within the prompt budget: with source when it fits,
/// else without it (noted), else none.
fn last_message(
  input: &Input<'_>,
  files: &[(&ShapedFile, &Package)],
  paths: &[&str],
  mask: &Mask,
  (prefix, reviews): (&Prefix, &mut Reviews),
) -> Option<String> {
  let prepass: Vec<_> = input
    .prepass
    .findings
    .iter()
    .filter(|finding| paths.contains(&finding.path.as_str()))
    .collect();
  let with_source = prompt::chunk(files, &prepass, input.memory, mask, true);
  if prompt::fits(prefix, &with_source) {
    return Some(with_source);
  }
  reviews.notes.push(format!(
    "surrounding source left out of a {}-file chunk: prompt over budget",
    paths.len()
  ));
  let bare = prompt::chunk(files, &prepass, input.memory, mask, false);
  prompt::fits(prefix, &bare).then_some(bare)
}

pub(crate) fn message<'a>(role: &'a str, content: &'a str) -> Message<'a> {
  Message { role, content }
}

fn send(
  reviews: &mut Reviews,
  paths: &[&str],
  messages: &[Message<'_>],
  call: &Call<'_>,
  ledger: &mut Ledger<'_>,
) {
  let request = model::request(messages, call.schema, call.level);
  match ledger.complete(
    Stage::Review,
    call.client,
    &request,
    Instant::now() + call.deadline,
  ) {
    Ok(completion) => match model::parse(&completion) {
      Ok(output) => {
        if output.partial {
          reviews.notes.push(format!(
            "partial review: {} finding(s) dropped from a chunk",
            output.dropped
          ));
        }
        reviews.findings.extend(output.findings);
      }
      Err(_) => reviews.fail(paths, "unusable structured review", false),
    },
    Err(toolu_openrouter::Error::Timeout) => reviews.fail(paths, "deadline", true),
    Err(err) => reviews.fail(paths, &err.to_string(), false),
  }
}

#[cfg(test)]
#[path = "tests/chunks_test.rs"]
mod tests;
