use super::{Measured, RSS_BYTES, WALL_US, cpu_model, document, violations};
use crate::bench_hooks_report::Sample;

fn measured(wall_us: u64, max_rss_bytes: u64) -> Measured {
  Measured {
    runs: 30,
    wall_us,
    max_rss_bytes,
    cold: Sample {
      max_rss_bytes: 4_853_760,
      cpu_us: 0,
      wall_us: 3762,
    },
  }
}

#[test]
fn the_document_keeps_the_committed_shape_and_key_order() {
  let doc = document(&measured(2824, 4_743_168), "AMD EPYC".to_owned());
  let text = serde_json::to_string_pretty(&doc).unwrap();
  let expected = "{\n  \"schema\": \"toolu.statusline-benchmark/v1\",";
  assert!(text.starts_with(expected), "{text}");
  let value: serde_json::Value = serde_json::from_str(&text).unwrap();
  assert_eq!(value["runs"], 30);
  assert_eq!(value["warmup"], 3);
  assert_eq!(value["wallUs"], 2824);
  assert_eq!(value["coldFirstRender"]["wallUs"], 3762);
  assert_eq!(value["budget"]["maxRssBytes"], 6_291_456);
  let at = |key: &str| text.find(&format!("\"{key}\"")).unwrap();
  let keys = [
    "schema",
    "platform",
    "cpu",
    "runs",
    "warmup",
    "wallUs",
    "maxRssBytes",
  ];
  let positions: Vec<usize> = keys.iter().map(|key| at(key)).collect();
  assert!(positions.is_sorted(), "{text}");
  assert!(at("coldFirstRender") < at("budget"), "{text}");
}

#[test]
fn bench_statusline_over_budget_fails() {
  assert_eq!(
    violations(&measured(WALL_US, RSS_BYTES)),
    Vec::<String>::new()
  );
  let found = violations(&measured(WALL_US + 1, RSS_BYTES + 1));
  assert_eq!(found.len(), 2, "{found:?}");
  assert!(found[0].contains("wall p50 4001 us > budget 4000 us"));
  assert!(found[1].contains("rss p50 6291457 bytes"));
  // The CI render that crossed the old 5 MiB budget on unchanged code.
  assert_eq!(violations(&measured(1417, 5_341_184)), Vec::<String>::new());
}

#[test]
fn the_cpu_model_is_never_empty() {
  assert_ne!(cpu_model(), "");
}
