/**
 * SessionStart (#269): publish the jev wrapper at `<config root>/jev/jev.sh`,
 * a stable path the agent's shell can expand (`CLAUDE_PLUGIN_ROOT` reaches
 * hook processes only), then state the Jev mandate. This plugin owns its
 * mandate so first-session delivery depends on neither the core plugin, hook
 * ordering, nor the model choosing to load a skill; the UserPromptSubmit hook
 * repeats a shorter form. No network calls. Port of the bash `session-start.sh`.
 */
import { accessSync, constants } from "node:fs";
import { resolve } from "node:path";
import { envValue } from "@toolu/core/host";
import { publishWrapper, renderHookOutput, sessionContext } from "@toolu/core/startup";

const PLUGIN = resolve(import.meta.dir, "../..");

function executable(path: string): boolean {
  try {
    accessSync(path, constants.X_OK);
    return true;
  } catch {
    return false;
  }
}

/** What stops the wrapper from working, space-prefixed, in the bash order; "" when nothing. */
function missingPrereqs(wrapper: string): string {
  const path = envValue(process.env, "PATH") ?? "";
  const missing = ["jq", "curl"].filter((tool) => Bun.which(tool, { PATH: path }) === null);
  if (envValue(process.env, "TYPESAFE_API_KEY") === undefined) missing.push("TYPESAFE_API_KEY");
  if (!executable(wrapper)) missing.push("executable-wrapper");
  return missing.map((item) => ` ${item}`).join("");
}

function mandate(wrapper: string): string {
  const missing = missingPrereqs(wrapper);
  if (missing !== "") {
    return `Jev unavailable (missing:${missing}). Set TYPESAFE_API_KEY in the agent's launch environment and install curl/jq. Jev is mandatory on every task once available; until then, state the limitation once per task and use an explicit reasoning/evidence fallback; never invent a Jev result. Do not read credentials from .env.`;
  }
  return `Jev is mandatory on every task containing semantic decisions. After initial exploration, identify useful judgments over supplied evidence; you MUST call "${wrapper}" before the decision it informs. Reassess after new evidence, failed hypotheses, or changed requirements. Batch independent questions in one ask call. Reuse unchanged evidence and questions rather than repeating calls. If a task has no semantic decision, say so in one sentence rather than skipping silently. Syntax and linked examples: ${PLUGIN}/skills/jev/SKILL.md. Keep exact rules, tests, and code verification deterministic. On service failure, state the limitation and use an explicit evidence fallback. Jev never replaces tests or authorization.`;
}

const result = publishWrapper({
  plugin: "jev",
  source: resolve(PLUGIN, "skills/jev/scripts/jev.sh"),
  dir: "jev",
  name: "jev.sh",
});
if (result.status === "link-failed") {
  process.stderr.write(`jev: cannot publish ${result.path}\n`);
} else if (result.status === "published" || result.status === "kept-user-file") {
  process.stdout.write(
    renderHookOutput(sessionContext("SessionStart", mandate(result.path)), false),
  );
}
