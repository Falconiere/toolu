use super::World;

#[test]
fn the_first_checkpoint_is_one_interval_out() {
  let world = World::new(1_000);
  assert_eq!(world.checkpoint_at_ms, 1_000 + super::CHECKPOINT_MS);
  assert_eq!(world.max_parallel, 1);
}
