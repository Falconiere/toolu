use serde_json::json;

use super::sample_of;
use crate::bench_hooks_report::Sample;

#[test]
fn a_clean_report_is_a_sample_with_user_plus_system_cpu() {
  let report = json!({"version": 1, "command": ["true"], "exitCode": 0, "signal": null,
    "wallUs": 900, "userUs": 300, "sysUs": 200, "maxRssBytes": 4096});
  assert_eq!(
    sample_of(&report).unwrap(),
    Sample {
      max_rss_bytes: 4096,
      cpu_us: 500,
      wall_us: 900
    }
  );
}

#[test]
fn a_failed_or_killed_hook_is_never_a_sample() {
  let failed =
    json!({"exitCode": 2, "signal": null, "wallUs": 1, "userUs": 1, "sysUs": 1, "maxRssBytes": 1});
  assert_eq!(
    sample_of(&failed).unwrap_err(),
    "the hook ended with exit 2 signal null"
  );
  let killed = json!({"exitCode": null, "signal": 9});
  assert_eq!(
    sample_of(&killed).unwrap_err(),
    "the hook ended with exit null signal 9"
  );
  let partial = json!({"exitCode": 0});
  assert_eq!(sample_of(&partial).unwrap_err(), "report lacks maxRssBytes");
}
