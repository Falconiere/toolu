use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use super::pause;

#[test]
fn stopped_subscription_does_not_pause() {
  let stop = Arc::new(AtomicBool::new(true));
  pause(&stop);
}
