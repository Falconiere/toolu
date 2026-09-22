import { expect, test } from "bun:test";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createPermissionEvaluateHandler } from "../evaluate.ts";
import type { PermissionEvaluationEvent } from "../permission-map.ts";

const tmpBase = process.env.TMPDIR ?? "/tmp";

function repoRoot(): string {
  return join(dirname(fileURLToPath(import.meta.url)), "../../../../..");
}

async function protectedEnvProject(): Promise<{ projectRoot: string; envPath: string }> {
  const projectRoot = await mkdtemp(join(tmpBase, "toolu-oc-eval-"));
  const envPath = join(projectRoot, ".env");
  await writeFile(envPath, "SECRET=1\n", "utf8");
  await mkdir(join(projectRoot, ".opencode"), { recursive: true });
  await writeFile(
    join(projectRoot, ".opencode/toolu.config.json"),
    JSON.stringify({
      version: 1,
      gates: { protectedFiles: { mode: "block" } },
    }),
    "utf8",
  );
  return { projectRoot, envPath };
}

function editEvent(envPath: string): PermissionEvaluationEvent {
  return {
    sessionID: "sess_eval_1",
    action: "edit",
    resources: [envPath],
    effect: "allow",
    metadata: { toolCallId: "call_eval_1" },
  };
}

test("AC-2: evaluate handler + real bridge on protected .env yields deny", async () => {
  const root = repoRoot();
  const { projectRoot, envPath } = await protectedEnvProject();
  const handler = createPermissionEvaluateHandler({
    repoRoot: root,
    bridgeContext: {
      cwd: projectRoot,
      projectRoot,
      worktree: projectRoot,
      host: "opencode",
    },
    env: {
      TOOLU_SETTINGS_DIR: join(root, "plugins/toolu/settings"),
      TOOLU_HOST_OVERRIDE: "opencode",
      TOOLU_PROJECT_CONFIG_DIRNAME: ".opencode",
    },
  });

  const event = editEvent(envPath);
  await handler(event);

  expect(event.effect).toBe("deny");
});

test("AC-3: runtime_failure from bad repoRoot maps to deny", async () => {
  const { projectRoot, envPath } = await protectedEnvProject();
  const handler = createPermissionEvaluateHandler({
    repoRoot: join(tmpBase, "toolu-nonexistent-repo-root"),
    bridgeContext: {
      cwd: projectRoot,
      projectRoot,
      worktree: projectRoot,
      host: "opencode",
    },
  });

  const event = editEvent(envPath);
  await handler(event);

  expect(event.effect).toBe("deny");
  expect(typeof event.message).toBe("string");
  expect(event.message && event.message.length > 0).toBe(true);
});
