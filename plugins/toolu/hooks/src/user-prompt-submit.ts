/**
 * UserPromptSubmit (#263): per-prompt hints for toolu. Port of the bash
 * `user-prompt-submit.sh` with byte-identical output: nothing for trivial
 * replies and slash commands, a block for a one-word verb, otherwise the
 * intent hint, the nudges, the project's `context.sh` and a failing-gate
 * reminder joined with ` | `. Context-only: any failure exits 0.
 */
import { join } from "node:path";
import { enabled, loadConfig } from "@toolu/core/config";
import { detectHost, projectDirname } from "@toolu/core/host";
import { renderHookOutput } from "@toolu/core/startup";
import {
  asciiLower,
  gitToplevel,
  jqAlt,
  member,
  onPath,
  parseStdin,
} from "./lifecycle/bash-compat.ts";
import { failingGateHint, projectContext } from "./lifecycle/project-context.ts";
import { mentionsGateTopic, promptGate, promptHints } from "./lifecycle/prompt-hints.ts";

const BLOCK = {
  decision: "block",
  reason: "Prompt too vague - specify what file/feature/error needs attention",
};

async function main(): Promise<void> {
  const env = process.env;
  const host = detectHost({ env });
  const config = loadConfig({ env, host });
  const input = await Bun.stdin.text();
  if (!enabled(config, "hooks", "user-prompt-submit")) return;
  const prompt = jqAlt(member(parseStdin(input), "prompt"), "");
  if (prompt === "") return;

  const lower = asciiLower(prompt);
  const gate = promptGate(lower);
  if (gate === "skip") return;
  if (gate === "block") {
    process.stdout.write(renderHookOutput(BLOCK, true));
    return;
  }

  const cwd = process.cwd();
  const root = gitToplevel(cwd, env) || cwd;
  const hostDir = projectDirname({ env, host });
  const gateHint = mentionsGateTopic(lower)
    ? undefined
    : failingGateHint(join(root, hostDir, "tmp"));
  const path = env.PATH ?? "";
  const astGrep =
    (onPath("sg", path) || onPath("ast-grep", path)) && enabled(config, "skills", "ast-grep");
  const parts = promptHints(prompt, lower, {
    astGrep,
    research: enabled(config, "agents", "research-agent"),
  });
  const project = projectContext(join(root, hostDir, "context.sh"), prompt, cwd, env);
  if (project !== "") parts.push(project);
  if (gateHint !== undefined) parts.push(gateHint);
  process.stdout.write(
    renderHookOutput(
      {
        hookSpecificOutput: {
          hookEventName: "UserPromptSubmit",
          additionalContext: parts.join(" | "),
        },
      },
      true,
    ),
  );
}

try {
  await main();
} catch (error) {
  process.stderr.write(`toolu user-prompt-submit: ${String(error)}\n`);
}
