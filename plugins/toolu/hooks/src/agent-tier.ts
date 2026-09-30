/**
 * PreToolUse for `spawn_agent|Agent|Task` (#262): delegation telemetry and the
 * plan-step model-tier check, as `agent-tier.sh` ran it. The matcher is loose
 * on purpose (it also matches TaskCreate, TaskUpdate, ...); the hook acts on
 * exactly `Agent`, `Task` and `spawn_agent`. Unlike the other PreToolUse
 * entries it fails open: a crash here must never block a delegation.
 */
import { agentTierHook } from "@toolu/core/gates/agent-tier";

try {
  const result = agentTierHook(await Bun.stdin.text());
  process.stdout.write(result.stdout);
  process.stderr.write(result.stderr);
  process.exitCode = result.exitCode;
} catch {
  process.exitCode = 0;
}
