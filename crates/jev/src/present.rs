//! A reply or an error as the `Outcome` of a verb. The binary adds the line
//! feed after each line and wraps a failure under `--json`, so nothing here does.

use toolu_jev_client::{Error, Reply};
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::json::ordered::Ordered;

use crate::call::CallError;

/// The answers as compact JSON in question order, or the whole body pretty-printed.
pub(crate) fn reply(reply: &Reply, raw: bool) -> Outcome {
  let Ok(body) = Ordered::parse(&reply.body) else {
    return error(&Error::InvalidResponse);
  };
  if raw {
    return Outcome::data(body.to_text(true));
  }
  let answers = reply
    .answers
    .iter()
    .map(|(id, _)| Some((id.clone(), body.get("answers")?.get(id)?.clone())))
    .collect::<Option<Vec<_>>>();
  match answers {
    Some(answers) => Outcome::data(Ordered::Object(answers).to_text(false)),
    None => error(&Error::InvalidResponse),
  }
}

/// A client failure: a terminal HTTP status prints its body, the rest `jev: `
/// and the client's message. A timeout may be retried.
pub(crate) fn error(err: &Error) -> Outcome {
  match err {
    Error::Http { body, .. } if !body.is_empty() => {
      Outcome::failed(Exit::Failure, body.trim_end_matches('\n').to_owned())
    }
    Error::Timeout => Outcome::failed(Exit::TempFail, format!("jev: {err}")),
    Error::MissingKey
    | Error::KeyLineBreak
    | Error::InvalidQuestion(_)
    | Error::Http { .. }
    | Error::Transport(_)
    | Error::InvalidResponse => Outcome::failed(Exit::Failure, format!("jev: {err}")),
  }
}

/// A call that could not be built.
pub(crate) fn call_error(err: &CallError) -> Outcome {
  match err {
    CallError::Client(err) => error(err),
    CallError::Input(message) => Outcome::failed(Exit::Failure, format!("jev: {message}")),
  }
}

#[cfg(test)]
#[path = "tests/present_test.rs"]
mod tests;
