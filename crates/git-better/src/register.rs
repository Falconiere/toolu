//! `SessionStart` registration: publish the manifests that enable this plugin's
//! compiled-in rules in toolu's registry.

use toolu_engine::quality::publish;
use toolu_protocol::host::Host;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::registry::RegistryEvent;
use toolu_runtime::registry::manifest::ModuleManifest;

use crate::{SPEC, post, pre};

/// Each rule's event, name and tool matcher.
const RULES: [(RegistryEvent, &str, &str); 2] = [
  (RegistryEvent::ToolPre, pre::NAME, "Bash|Shell"),
  (RegistryEvent::ToolPost, post::NAME, "Bash|Shell"),
];

/// Publish every manifest. Never fails the session: an error is a `systemMessage`.
pub fn register(env: &Env, host: Host) -> Outcome {
  let roots = Roots::new(env.clone(), Some(host));
  let failed = RULES.iter().find_map(|(event, name, matcher)| {
    let manifest = ModuleManifest {
      version: 1,
      spec: SPEC.to_owned(),
      name: (*name).to_owned(),
      event: *event,
      matcher: (*matcher).to_owned(),
    };
    publish(env, &roots, &manifest).err()
  });
  Outcome::notice(failed.map(|error| format!("git-better registration failed: {error}")))
}

#[cfg(test)]
#[path = "tests/register_test.rs"]
mod tests;
