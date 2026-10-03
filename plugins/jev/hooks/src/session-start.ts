/**
 * SessionStart (#269): publish the jev wrapper at `<config root>/jev/jev.sh`,
 * a stable path the agent's shell can expand (`CLAUDE_PLUGIN_ROOT` reaches
 * hook processes only), then state the Jev mandate. This plugin owns its
 * mandate so first-session delivery depends on neither the core plugin, hook
 * ordering, nor the model choosing to load a skill; the UserPromptSubmit hook
 * repeats a shorter form. No network calls. Port of the bash `session-start.sh`.
 * On OpenCode the system transform already carries the mandate on every request,
 * so compaction only relinks the wrapper (#350).
 */
import { accessSync, constants } from "node:fs";
import { resolve } from "node:path";
import { publishWrapper, renderHookOutput, sessionContext } from "@toolu/core/startup";
import { invocation, credentialNotice, onOpencode, skillReference } from "./jev/availability.ts";

const PLUGIN = resolve(import.meta.dir, "../..");

function executable(path: string): boolean {
  try {
    accessSync(path, constants.X_OK);
    return true;
  } catch {
    return false;
  }
}

function mandate(wrapper: string): string {
  if (!executable(wrapper)) {
    return "Jev unavailable: published wrapper is not executable. Repair the Jev plugin installation. Until then, state the limitation once per task and use an explicit reasoning/evidence fallback; never invent a Jev result. Do not read credentials from .env.";
  }
  return `${credentialNotice()}Jev is mandatory on every task containing semantic decisions. After initial exploration, identify useful judgments over supplied evidence; you MUST call ${invocation(wrapper)} before the decision it informs. Published bundles use the hook's resolved Bun executable and do not require bun on PATH. Reassess after new evidence, failed hypotheses, or changed requirements. Batch independent questions in one ask call. Reuse unchanged evidence and questions rather than repeating calls. If a task has no semantic decision, say so in one sentence rather than skipping silently. Syntax and linked examples: ${skillReference(PLUGIN)}. Keep exact rules, tests, and code verification deterministic. On service failure, state the limitation and use an explicit evidence fallback. Jev never replaces tests or authorization.`;
}

/** SessionStart stdin names its trigger; unreadable stdin counts as a plain start. */
async function compacting(): Promise<boolean> {
  let input: unknown;
  try {
    input = JSON.parse(await Bun.stdin.text());
  } catch {
    return false;
  }
  return (
    input !== null && typeof input === "object" && "source" in input && input.source === "compact"
  );
}

const quiet = onOpencode() && (await compacting());
const result = publishWrapper({
  plugin: "jev",
  source: resolve(PLUGIN, "hooks/dist/jev.js"),
  dir: "jev",
  name: "jev.sh",
});
if (result.status === "link-failed") {
  process.stderr.write(`jev: cannot publish ${result.path}\n`);
} else if (!quiet && (result.status === "published" || result.status === "kept-user-file")) {
  process.stdout.write(
    renderHookOutput(sessionContext("SessionStart", mandate(result.path)), false),
  );
}
