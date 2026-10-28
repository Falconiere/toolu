//! The public model-output contract that orchestration and anchoring consume.

use toolu_code_review::model::{ModelFinding, ModelOutput, parse};
use toolu_openrouter::{Completion, Usage};

#[test]
fn a_downstream_crate_can_read_valid_findings_and_partial_status() {
  let completion = Completion {
    content: "{\"findings\":[{\"path\":\"src/lib.rs\",\"line\":1,\"end_line\":1,\"side\":\"new\",\"severity\":\"high\",\"category\":\"correctness\",\"claim\":\"panic\",\"quote\":\"items[0]\",\"suggestion\":null,\"prior_id\":null}]}".into(),
    finish_reason: Some("stop".into()),
    model: "deepseek/deepseek-v4-pro-0813".into(),
    provider: Some("Parasail".into()),
    quantization: Some("fp8".into()),
    usage: Usage { prompt_tokens: 1, completion_tokens: 2, reasoning_tokens: 0, cost_usd: None },
  };
  let output: ModelOutput = parse(&completion).expect("output");
  let finding: &ModelFinding = &output.findings[0];
  assert_eq!(finding.path, "src/lib.rs");
  assert!(!output.partial);
}
