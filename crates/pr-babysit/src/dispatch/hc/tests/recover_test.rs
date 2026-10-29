use super::*;
use crate::dispatch::sandbox::{Core, sandbox};

#[test]
fn only_a_proven_prelaunch_absence_can_create_a_terminal() {
  let error = failure(&hc::Error::new("not_found", "absent"));
  let group = json!({"hcName":"saved"});
  assert!(can_launch(&group, &error));
  assert!(!can_launch(
    &group,
    &failure(&hc::Error::new("transport", "offline"))
  ));
  assert!(!can_launch(
    &json!({"hcName":"saved","hcAddress":"t1"}),
    &error
  ));
  assert!(!can_launch(
    &json!({"hcName":"saved","hcSendStarted":true}),
    &error
  ));
}

#[test]
fn recovery_launches_the_saved_identity_once_before_first_delivery() {
  let sb = sandbox();
  let core = Core::new(&sb.env).unwrap();
  let mut options = sb.options(false);
  options.env = &core.env;
  let mut dispatcher = Dispatcher::new(&options).unwrap();
  let group = json!({"seq":1,"status":"launching","hcName":"saved",
    "hcOwner":"toolu.owner=recovery","hcSession":"test","brief":"/brief.md",
    "hcCommand":["sh","-c","printf '\\033]7501;state=idle\\007'; while read line; do printf 'seen:%s\\n' \"$line\"; done"]});
  dispatcher
    .save(|state| {
      state["herdrWorktree"] = json!({"path":sb.root,"multiplexer":"herdr-app"});
      state["fixer"] = json!({"status":"running","groups":[group.clone()]});
    })
    .unwrap();
  dispatcher.recover_hc(1, &group).unwrap();
  let saved = dispatcher.group(1).unwrap();
  assert_eq!(saved["status"], "running");
  assert_eq!(saved["hcSendConfirmed"], true);
  let address = saved["hcAddress"].as_str().unwrap();
  let client = Client::new(&core.env);
  hc::require_owner(&client.get(address).unwrap(), "recovery").unwrap();
  dispatcher.recover_hc(1, &saved).unwrap();
  assert_eq!(dispatcher.group(1).unwrap()["hcAddress"], address);
  let terminals = client
    .call(&["ls"], std::time::Duration::from_secs(10))
    .unwrap();
  assert_eq!(terminals["terminals"].as_array().unwrap().len(), 1);
  client.kill_owned(address, "recovery").unwrap();
}
