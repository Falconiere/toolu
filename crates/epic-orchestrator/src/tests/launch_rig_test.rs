//! A launch test rig: a temp epic state on the #248 graph, a resource root, a
//! herdr socket fixture and an origin-backed checkout for live launches.

use std::path::{Path, PathBuf};
use std::time::Duration;

use toolu_runtime::env::Env;

use crate::graph::Graph;
use crate::herdr::Client;
use crate::herdr_fixture::HerdrFixture;
use crate::launch::{Ctx, Herdr, LaunchOpts};
use crate::paths::Paths;

/// Everything one launch test touches.
pub(crate) struct Rig {
  pub tmp: tempfile::TempDir,
  pub herdr: HerdrFixture,
  pub env: Env,
  pub paths: Paths,
  pub graph: Graph,
}

/// The Claude transcript an agent writes in its worktree (`{claude}` is the
/// config directory): one record naming its cwd and session.
const CLAUDE_AGENT: &str = r#"slug=$(printf %s "$PWD" | tr -c 'A-Za-z0-9' '-'); dir="{claude}/projects/$slug"; mkdir -p "$dir"; printf '{"cwd":"%s","sessionId":"0199a272-0b4e-7cc3-8c4f-071c81275655"}\n' "$PWD" > "$dir/s.jsonl"; exec sleep 600"#;

/// The session [`CLAUDE_AGENT`] records.
pub(crate) const CLAUDE_SESSION: &str = "0199a272-0b4e-7cc3-8c4f-071c81275655";

impl Rig {
  /// A rig whose graph state lives in a temp directory.
  pub(crate) fn new() -> Self {
    let tmp = tempfile::tempdir().expect("temp");
    let herdr = HerdrFixture::start();
    let mut graph = crate::support::graph_248();
    graph.state_dir = tmp.path().join("state").display().to_string();
    let env = herdr
      .env()
      .with("HOME", &tmp.path().join("home").display().to_string())
      .with(
        "TOOLU_RESOURCE_HOME",
        &tmp.path().join("resources").display().to_string(),
      )
      .with(
        "CLAUDE_CONFIG_DIR",
        &tmp.path().join("claude").display().to_string(),
      )
      .with(
        "CODEX_HOME",
        &tmp.path().join("codex").display().to_string(),
      );
    let paths = Paths::at(tmp.path().join("engine"));
    Self {
      tmp,
      herdr,
      env,
      paths,
      graph,
    }
  }

  /// The launch context, with no GitHub.
  pub(crate) fn ctx(&self) -> Ctx<'_> {
    Ctx {
      env: &self.env,
      herdr: Herdr {
        client: Client::new(),
        paths: &self.paths,
        env: &self.env,
      },
      gh: Err("GitHub is not reachable in this test".to_owned()),
      session_wait: Duration::from_secs(5),
      shell_wait: Duration::from_secs(2),
    }
  }

  /// Agents started from now on record a Claude session as they start.
  pub(crate) fn claude_agents(&self) {
    let claude = self.tmp.path().join("claude").display().to_string();
    self.herdr.state().agent_script = CLAUDE_AGENT.replace("{claude}", &claude);
  }

  /// The state directory.
  pub(crate) fn state(&self) -> PathBuf {
    PathBuf::from(&self.graph.state_dir)
  }

  /// Point `key` at a real checkout whose origin is a local bare repository,
  /// and register it with herdr; the checkout.
  pub(crate) fn checkout(&mut self, key: &str) -> PathBuf {
    let origin = self.tmp.path().join("origin.git");
    let checkout = self.tmp.path().join("checkout");
    crate::support::origin_with_clone(&origin, &checkout, &[("a.txt", "one\n")]);
    let found = self
      .graph
      .issues
      .iter_mut()
      .find(|issue| issue.key == key)
      .expect("issue");
    found.checkout = Some(checkout.display().to_string());
    self.herdr.add_repo(&checkout);
    checkout
  }

  /// The record of `key`, parsed.
  pub(crate) fn record(&self, key: &str) -> serde_json::Value {
    let path = self.state().join("issues").join(format!("{key}.json"));
    serde_json::from_str(&std::fs::read_to_string(path).expect("record")).expect("json")
  }

  /// Write `body` as the record of `key`.
  pub(crate) fn write_record(&self, key: &str, body: &serde_json::Value) {
    let path = self.state().join("issues").join(format!("{key}.json"));
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
    std::fs::write(path, body.to_string()).expect("record");
  }

  /// The resource leases.
  pub(crate) fn leases(&self) -> usize {
    let state =
      toolu_engine::resources::store::read_resource_state(&self.tmp.path().join("resources"))
        .expect("resources");
    toolu_engine::resources::store::leases(&state).len()
  }
}

/// The default flags: unattended, live, not forced.
pub(crate) fn opts() -> LaunchOpts {
  LaunchOpts {
    permission_mode: "auto".to_owned(),
    ..LaunchOpts::default()
  }
}

/// Every file under `dir` with its bytes, for byte-identical checks.
pub(crate) fn snapshot_tree(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
  let mut out = Vec::new();
  let Ok(entries) = std::fs::read_dir(dir) else {
    return out;
  };
  for entry in entries.filter_map(Result::ok) {
    let path = entry.path();
    if path.is_dir() {
      out.extend(snapshot_tree(&path));
    } else {
      out.push((path.clone(), std::fs::read(&path).unwrap_or_default()));
    }
  }
  out.sort();
  out
}
