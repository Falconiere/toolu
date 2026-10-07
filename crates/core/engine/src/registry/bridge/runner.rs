//! The JavaScript the Bun bridge runs with `bun --no-install -e`, and the request
//! it reads on stdin. The runner imports each module and checks it the way
//! `runContract` and `DecisionSchema` do in `registry-run.ts`, with the same
//! messages, and prints one marked line per module.

use toolu_runtime::json::jq_text;
use toolu_runtime::json::ordered::Ordered;

use crate::registry::Entry;

/// The prefix of a result line on the runner's stdout.
pub(crate) const MARK: &str = "toolu-bridge:";

/// The runner. It writes with `writeSync`, so a module that exits the process
/// cannot lose the lines of the modules before it.
pub(crate) const RUNNER: &str = r#"const MARK = "toolu-bridge:";
const { inspect } = await import("node:util");
const { writeSync } = await import("node:fs");
const request = JSON.parse(await Bun.stdin.text());
const ctx = { ...request.ctx, env: process.env };
const isObject = (v) => typeof v === "object" && v !== null && !Array.isArray(v);
const text = (v) => typeof v === "string" && v.length > 0;
const CODES = ["timeout", "spawn", "parse", "truncated", "cancelled", "nonzero"];
function decisionOf(d) {
  if (!isObject(d)) return undefined;
  const { kind, reason, message, code } = d;
  if (kind === "allow") return { kind };
  if (kind === "ask" || kind === "deny" || kind === "post_block") return text(reason) ? { kind, reason } : undefined;
  if (kind === "advisory") return text(message) ? { kind, message } : undefined;
  if (kind === "runtime_failure") return text(reason) && CODES.includes(code) ? { kind, reason, code } : undefined;
  return undefined;
}
async function run(module) {
  const loaded = await import(module.path);
  const exported = isObject(loaded) ? loaded.default : undefined;
  const { spec, name, event, run } = isObject(exported) ? exported : {};
  if (typeof spec !== "string" || typeof name !== "string" || (event !== "tool/pre" && event !== "tool/post") || typeof run !== "function") {
    throw new Error("default export is not a registry module");
  }
  const want = { spec: module.spec, name: module.name, event: request.registryEvent };
  if (spec !== want.spec || name !== want.name || event !== want.event) {
    throw new Error(`contract mismatch: exports ${JSON.stringify({ spec, name, event })}, file and directory want ${JSON.stringify(want)}`);
  }
  const result = await run.call(exported, request.event, ctx);
  const decision = decisionOf(result);
  if (decision === undefined) throw new Error(`invalid decision: ${inspect(result)}`);
  return decision;
}
for (const module of request.modules) {
  let line;
  try {
    line = { file: module.file, decision: await run(module) };
  } catch (error) {
    line = { file: module.file, error: error instanceof Error ? error.message : String(error) };
  }
  writeSync(1, `\n${MARK}${JSON.stringify(line)}\n`);
  if (line.decision !== undefined && line.decision.kind === request.stop) break;
}
"#;

/// What one batch is asked to run.
pub(crate) struct Request<'r> {
  /// `deny` before a tool, `post_block` after it: the kind that ends the walk.
  pub(crate) stop: &'static str,
  /// `tool/pre` or `tool/post`: the directory's event, which exports must declare.
  pub(crate) registry_event: &'static str,
  /// The event, in `toolEvent`'s key order.
  pub(crate) event: &'r Ordered,
  /// The context, without `env`.
  pub(crate) ctx: &'r Ordered,
}

/// The request text for `modules`.
pub(crate) fn request_text(request: &Request<'_>, modules: &[&Entry]) -> String {
  let text = |value: &str| Ordered::String(value.to_owned());
  let modules = modules
    .iter()
    .map(|entry| {
      Ordered::Object(vec![
        ("file".to_owned(), text(&entry.file)),
        ("path".to_owned(), text(&entry.path.to_string_lossy())),
        ("spec".to_owned(), text(&entry.spec)),
        ("name".to_owned(), text(&entry.name)),
      ])
    })
    .collect();
  let body = Ordered::Object(vec![
    ("stop".to_owned(), text(request.stop)),
    ("registryEvent".to_owned(), text(request.registry_event)),
    ("event".to_owned(), request.event.clone()),
    ("ctx".to_owned(), request.ctx.clone()),
    ("modules".to_owned(), Ordered::Array(modules)),
  ]);
  jq_text(&body, false)
}

#[cfg(test)]
#[path = "tests/runner_test.rs"]
mod tests;
