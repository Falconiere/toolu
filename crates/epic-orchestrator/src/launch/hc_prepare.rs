//! Durable worktree preparation and explicit hc recovery.

use super::Ctx;
use super::attempt::Attempt;
use super::checkout::prepare_checkout;
use super::failure::Failure;
use super::hc::{failed, git, local_worktree};
use super::record::{Record, text};
use serde_json::{Value, json};
use std::path::Path;
use toolu_mux::hc::{Client, require_owner};

impl Attempt<'_> {
  pub(super) fn hc_prepare(
    &mut self,
    record: &mut Record,
    ctx: &mut Ctx<'_>,
  ) -> Result<(), Failure> {
    if text(record, "terminal_name").is_none() {
      self.hc_initialize(record, ctx)?;
    }
    self.hc_remote_prepare(record, ctx)?;
    let checkout = text(record, "checkout").unwrap_or("").to_owned();
    let path = text(record, "worktree").unwrap_or("").to_owned();
    let revision = text(record, "expected_base").unwrap_or("").to_owned();
    local_worktree(ctx.env, &checkout, &self.issue.branch, &revision, &path)?;
    let canonical = std::fs::canonicalize(&path).map_err(|e| Failure::unknown(e.to_string()))?;
    record.insert("worktree".into(), json!(canonical.display().to_string()));
    let owner = self
      .owner
      .as_ref()
      .ok_or_else(|| Failure::refusal("hc ownership missing"))?;
    toolu_engine::resources::binding::bind_worktree(
      &owner.root,
      &canonical,
      &self.issue.key,
      &self.graph.state_dir,
    )
    .map_err(Failure::unknown)?;
    record.insert("hc_prepared".into(), json!(true));
    self.persist(record, "starting")
  }

  fn hc_initialize(&mut self, record: &mut Record, ctx: &mut Ctx<'_>) -> Result<(), Failure> {
    self.initialize(record)?;
    record.insert("model".into(), json!(self.host.model));
    record.insert("effort".into(), json!(self.host.effort));
    record.insert("bypass".into(), json!(!self.opts.safe));
    record.insert("permission_mode".into(), json!(self.opts.permission_mode));
    let (checkout, base) = prepare_checkout(
      self.graph,
      self.issue,
      false,
      &mut self.log,
      (ctx.env, ctx.gh.clone()),
    )?;
    let owner = self
      .owner
      .as_ref()
      .ok_or_else(|| Failure::refusal("hc ownership missing"))?;
    let id = super::digest::md5_hex(format!("{}:{}", owner.token, self.started_at).as_bytes());
    let path = Path::new(&checkout)
      .join("..")
      .join(format!("toolu-{}", id.get(..24).unwrap_or(&id)))
      .display()
      .to_string();
    let revision = git(
      ctx.env,
      &checkout,
      &["rev-parse", &format!("origin/{base}^{{commit}}")],
    )?;
    for (key, value) in [
      ("checkout", checkout.as_str()),
      ("base", base.as_str()),
      ("worktree", path.as_str()),
      (
        "terminal_name",
        &format!("toolu-{}", id.get(..24).unwrap_or(&id)),
      ),
      ("terminal_owner", &format!("toolu.owner={}", owner.token)),
      ("hc_run", id.as_str()),
      ("expected_base", revision.as_str()),
      ("hc_session", ctx.env.get("HC_SESSION").unwrap_or("default")),
    ] {
      record.insert(key.into(), json!(value));
    }
    self.persist(record, "starting")
  }

  pub(super) fn hc_recover(&self, record: &mut Record, ctx: &Ctx<'_>) -> Result<(), Failure> {
    if text(record, "hc_session")
      .is_some_and(|s| s != ctx.env.get("HC_SESSION").unwrap_or("default"))
    {
      return Err(Failure::refusal(
        "recorded HC_SESSION differs; restore it before recovery",
      ));
    }
    if self.opts.replace()
      && let Some(address) = text(record, "terminal_address")
    {
      let terminal = Client::new(ctx.env).get(address).map_err(failed)?;
      require_owner(&terminal, text(record, "terminal_owner").unwrap_or("")).map_err(failed)?;
      if terminal.get("state").and_then(Value::as_str) != Some("finished") {
        return Err(Failure::refusal(
          "replacement requires verified old terminal shutdown",
        ));
      }
      if super::hc::remote_worker(record) {
        return Err(Failure::refusal(
          "inspect remote replacement before dispatch",
        ));
      }
      let id = super::digest::md5_hex(
        format!(
          "{}:{}",
          text(record, "terminal_owner").unwrap_or(""),
          self.started_at
        )
        .as_bytes(),
      );
      record.insert(
        "terminal_name".into(),
        json!(format!("toolu-{}", id.get(..24).unwrap_or(&id))),
      );
      record.remove("terminal_address");
      record.insert("prompt_state".into(), json!("pending"));
      self.persist(record, "starting")?;
    } else if self.opts.reprompt() && text(record, "prompt_state") == Some("acknowledged") {
      record.insert("prompt_state".into(), json!("pending"));
      self.persist(record, "starting")?;
    }
    Ok(())
  }
}

#[cfg(test)]
#[path = "tests/hc_prepare_test.rs"]
mod tests;
