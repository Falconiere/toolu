//! What Jev is asked: the state and the questions, as JSON whose objects keep
//! their order, so a request carries the text `jev.ts` sends.

use toolu_runtime::json::ordered::Ordered;

use crate::Error;

/// The state Jev judges.
#[derive(Debug, Clone, PartialEq)]
pub struct State(Ordered);

impl State {
  /// `text` as a JSON string.
  pub fn text(text: &str) -> State {
    State(Ordered::String(text.to_owned()))
  }

  /// File or stdin text as `jev.ts` reads `--state @FILE` and `-s -`: trailing
  /// line feeds removed, then JSON when it is an object or an array, else a
  /// string.
  pub fn structured(text: &str) -> State {
    let text = text.trim_end_matches('\n');
    match Ordered::parse(text) {
      Ok(value @ (Ordered::Object(_) | Ordered::Array(_))) => State(value),
      Ok(Ordered::Null | Ordered::Bool(_) | Ordered::Number(_) | Ordered::String(_)) | Err(_) => {
        State::text(text)
      }
    }
  }

  /// The JSON value.
  pub(crate) fn value(&self) -> &Ordered {
    &self.0
  }
}

/// One question.
#[derive(Debug, Clone, PartialEq)]
pub struct Question(Ordered);

impl Question {
  /// A yes/no question; `yes` and `no` describe each answer when not empty.
  ///
  /// # Errors
  /// `InvalidQuestion` for empty instructions.
  pub fn noul(instructions: &str, yes: Option<&str>, no: Option<&str>) -> Result<Question, Error> {
    let mut fields = head("noul", instructions)?;
    let sides: Vec<(String, Ordered)> = [("true", yes), ("false", no)]
      .into_iter()
      .filter_map(|(side, text)| {
        let text = text.filter(|text| !text.is_empty())?;
        Some((side.to_owned(), Ordered::String(text.to_owned())))
      })
      .collect();
    if !sides.is_empty() {
      fields.push(("criteria".to_owned(), Ordered::Object(sides)));
    }
    Ok(Question(Ordered::Object(fields)))
  }

  /// Pick one of 2 to 255 options, each a key with an optional description.
  /// A repeated key keeps its first position and its last description.
  ///
  /// # Errors
  /// `InvalidQuestion` for empty instructions, an empty key, or a count
  /// outside 2 to 255.
  pub fn choice(instructions: &str, options: &[(&str, Option<&str>)]) -> Result<Question, Error> {
    let mut fields = head("choice", instructions)?;
    let mut criteria = Ordered::Object(Vec::new());
    for (key, description) in options {
      if key.is_empty() {
        return Err(Error::InvalidQuestion("a choice option needs a key".into()));
      }
      let description = description.map_or(Ordered::Null, |text| Ordered::String(text.to_owned()));
      criteria.set(key, description);
    }
    let count = match &criteria {
      Ordered::Object(entries) => entries.len(),
      Ordered::Null
      | Ordered::Bool(_)
      | Ordered::Number(_)
      | Ordered::String(_)
      | Ordered::Array(_) => 0,
    };
    bounded("choice", "options", count, 255)?;
    fields.push(("criteria".to_owned(), criteria));
    Ok(Question(Ordered::Object(fields)))
  }

  /// Rate on 2 to 10 levels, lowest first.
  ///
  /// # Errors
  /// `InvalidQuestion` for empty instructions or a count outside 2 to 10.
  pub fn score(instructions: &str, levels: &[&str]) -> Result<Question, Error> {
    let mut fields = head("score", instructions)?;
    bounded("score", "levels", levels.len(), 10)?;
    let levels = levels
      .iter()
      .map(|level| Ordered::String((*level).to_owned()))
      .collect();
    fields.push(("criteria".to_owned(), Ordered::Array(levels)));
    Ok(Question(Ordered::Object(fields)))
  }
}

fn head(kind: &str, instructions: &str) -> Result<Vec<(String, Ordered)>, Error> {
  if instructions.is_empty() {
    return Err(Error::InvalidQuestion(format!("{kind} needs instructions")));
  }
  Ok(vec![
    ("type".to_owned(), Ordered::String(kind.to_owned())),
    (
      "instructions".to_owned(),
      Ordered::String(instructions.to_owned()),
    ),
  ])
}

fn bounded(kind: &str, what: &str, count: usize, most: usize) -> Result<(), Error> {
  if count < 2 {
    return Err(Error::InvalidQuestion(format!(
      "{kind} needs at least 2 {what}"
    )));
  }
  if count > most {
    return Err(Error::InvalidQuestion(format!(
      "{kind} accepts at most {most} {what}"
    )));
  }
  Ok(())
}

/// Questions by id, in order; each id names its answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Questions(Vec<(String, Ordered)>);

impl Questions {
  /// One question under `id`.
  pub fn single(id: &str, question: Question) -> Questions {
    Questions(vec![(id.to_owned(), question.0)])
  }

  /// An `ask` payload: a non-empty JSON object of questions, sent as given.
  ///
  /// # Errors
  /// `InvalidQuestion` for invalid JSON, a value that is not an object, or an
  /// empty object.
  pub fn parse(text: &str) -> Result<Questions, Error> {
    let invalid = |reason: &str| Err(Error::InvalidQuestion(reason.to_owned()));
    match Ordered::parse(text) {
      Ok(Ordered::Object(entries)) if entries.is_empty() => invalid("questions must not be empty"),
      Ok(Ordered::Object(entries)) => Ok(Questions(entries)),
      Ok(
        Ordered::Null
        | Ordered::Bool(_)
        | Ordered::Number(_)
        | Ordered::String(_)
        | Ordered::Array(_),
      ) => invalid("questions must be a JSON object"),
      Err(_) => invalid("questions are not valid JSON"),
    }
  }

  /// The questions by id, in order.
  pub(crate) fn entries(&self) -> &[(String, Ordered)] {
    &self.0
  }

  /// The JSON object sent as `questions`.
  pub(crate) fn value(&self) -> Ordered {
    Ordered::Object(self.0.clone())
  }
}

#[cfg(test)]
#[path = "tests/question_test.rs"]
mod tests;
