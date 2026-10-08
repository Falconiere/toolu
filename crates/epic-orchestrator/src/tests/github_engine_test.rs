use super::{cause_name, rearm};
use crate::model::World;
use crate::watch::{Cause, Kind, Watch};

#[test]
fn a_rate_limit_rearms_unprocessed_watches_without_moving_their_deadlines() {
  let mut world = World::new(0);
  let key = "pr:Falconiere/toolu#501".to_owned();
  let mut watch = Watch::new(
    Kind::Pr {
      key: "toolu-447".to_owned(),
      repo: "Falconiere/toolu".to_owned(),
      number: 501,
    },
    0,
  );
  assert_eq!(watch.take_due(0), Some(Cause::Immediate));
  let deadline = watch.next_at_ms;
  world.watches.insert(key.clone(), watch);
  rearm(&mut world, &[(key.clone(), Cause::Scheduled)], 0);
  assert_eq!(world.watches.get(&key).expect("watch").next_at_ms, deadline);
  assert_eq!(
    world.watches.get_mut(&key).expect("watch").take_due(1),
    Some(Cause::Immediate)
  );
  assert_eq!(cause_name(Cause::Scheduled), "scheduled");
}
