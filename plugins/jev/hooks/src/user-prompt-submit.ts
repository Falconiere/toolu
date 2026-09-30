/** Repeat the Jev mandate on substantive prompts without a network call. */
import { accessSync, constants } from "node:fs";
import { resolve, join } from "node:path";
import { configRoot } from "@toolu/core/host";
import { bunOnPath, renderHookOutput, sessionContext } from "@toolu/core/startup";

const trivial =
  /^\s*(?:y|n|yes|no|ok|okay|sure|thanks|thank you|go ahead|looks good|lgtm|correct|exactly|right|done|nah|nope|yep|yup|continue)[.!?]?\s*$/i;
const plugin = resolve(import.meta.dir, "../..");
const wrapper = join(configRoot({ env: process.env }), "jev/jev.sh");

function executable(path: string): boolean {
  try {
    accessSync(path, constants.X_OK);
    return true;
  } catch {
    return false;
  }
}

async function main(): Promise<void> {
  let input: unknown;
  try {
    input = JSON.parse(await Bun.stdin.text());
  } catch {
    return;
  }
  if (input === null || typeof input !== "object" || Array.isArray(input)) return;
  const prompt = (input as Record<string, unknown>).prompt;
  if (typeof prompt !== "string" || !prompt || trivial.test(prompt)) return;
  if (!bunOnPath() || !process.env.TYPESAFE_API_KEY || !executable(wrapper)) return;
  const context = `Jev is mandatory for this task when it contains semantic decisions. After initial exploration, identify useful judgments over supplied evidence; you MUST call "${wrapper}" before the decision it informs. Reassess after new evidence, failed hypotheses, or changed requirements. Batch independent questions in one ask call. Reuse unchanged evidence and questions rather than repeating calls. If a task has no semantic decision, say so in one sentence rather than skipping silently. Syntax and linked examples: ${plugin}/skills/jev/SKILL.md. On failure, state the limitation and use an evidence fallback; Jev never replaces tests or authorization.`;
  process.stdout.write(renderHookOutput(sessionContext("UserPromptSubmit", context), false));
}

await main();
