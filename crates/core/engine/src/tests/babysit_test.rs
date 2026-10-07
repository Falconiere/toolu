use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use serde_json::json;

use crate::LinkError;
use crate::babysit::{BabysitTick, TickDecision, TickReport, TickRequest};

/// A tick that answers with a fixed decision, or fails for one repository.
struct Fixed(TickDecision);

impl BabysitTick for Fixed {
  fn tick(&self, request: &TickRequest) -> Result<TickReport, LinkError> {
    if request.repo == "o/broken" {
      return Err(LinkError::Failed(format!(
        "no state at {}",
        request.state_file.display()
      )));
    }
    Ok(TickReport {
      decision: self.0,
      result: json!({ "slot": format!("{}#{}", request.repo, request.number) }),
    })
  }
}

fn request(repo: &str) -> TickRequest {
  TickRequest {
    repo: repo.to_owned(),
    number: 460,
    state_file: PathBuf::from("/state/460.json"),
    now: Some("2026-10-07T04:00:00Z".to_owned()),
  }
}

#[test]
fn a_tick_runs_through_a_trait_object() {
  let tick: &dyn BabysitTick = &Fixed(TickDecision::Success);
  let report = tick.tick(&request("Falconiere/toolu")).expect("report");
  assert_eq!(report.decision, TickDecision::Success);
  assert_eq!(report.result["slot"], "Falconiere/toolu#460");
  assert_eq!(
    tick.tick(&request("o/broken")),
    Err(LinkError::Failed("no state at /state/460.json".to_owned()))
  );
}

#[test]
fn a_shared_tick_runs_on_another_thread() {
  let tick: Arc<dyn BabysitTick> = Arc::new(Fixed(TickDecision::KeepGoing));
  let worker = Arc::clone(&tick);
  let decision = thread::spawn(move || worker.tick(&request("o/r")).map(|report| report.decision))
    .join()
    .expect("thread");
  assert_eq!(decision, Ok(TickDecision::KeepGoing));
}
