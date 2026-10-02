import { expect, test } from "bun:test";
import { mkdir, mkdtemp, symlink, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { FOREIGN_HOST_VARS } from "../../host/runtime-env.ts";
import { createPermissionEvaluateHandler, gateEnv } from "../evaluate.ts";
import type { PermissionEvaluationEvent } from "../permission-map.ts";
import { bootstrapRuntime } from "../../bootstrap/runtime.ts";
import { selectPluginsByEnabledNames } from "../../select/resolve.ts";

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

test("AC-2: evaluate handler + core dispatcher on protected .env yields deny", async () => {
  const root = repoRoot();
  const { projectRoot, envPath } = await protectedEnvProject();
  const handler = createPermissionEvaluateHandler({
    repoRoot: root,
    configRoot: join(projectRoot, ".opencode", "toolu", "state"),
    permissionContext: {
      cwd: projectRoot,
      projectRoot,
      worktree: projectRoot,
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

test("#276: native protected-file deny works with bash and jq absent from PATH", async () => {
  const root = repoRoot();
  const { projectRoot, envPath } = await protectedEnvProject();
  const bin = await mkdtemp(join(tmpBase, "toolu-oc-bin-"));
  await symlink(Bun.which("git") ?? "/usr/bin/git", join(bin, "git"));
  await symlink(Bun.which("bun") ?? process.execPath, join(bin, "bun"));
  const handler = createPermissionEvaluateHandler({
    repoRoot: root,
    configRoot: join(projectRoot, ".opencode", "toolu", "state"),
    permissionContext: { cwd: projectRoot, projectRoot, worktree: projectRoot },
    env: {
      PATH: bin,
      TOOLU_SETTINGS_DIR: join(root, "plugins/toolu/settings"),
      TOOLU_HOST_OVERRIDE: "opencode",
      TOOLU_PROJECT_CONFIG_DIRNAME: ".opencode",
    },
  });
  const event = editEvent(envPath);
  await handler(event);
  expect(event.effect).toBe("deny");
  expect(event.message).toContain("protected");
});

test("#276: bundled ast-grep registry runs from isolated OpenCode root without bash", async () => {
  const root = repoRoot();
  const packageRoot = await mkdtemp(join(tmpBase, "toolu-oc-package-"));
  const staged = join(packageRoot, "plugins");
  const bundled = Bun.spawnSync(
    [process.execPath, join(root, "tools/toolu-opencode/scripts/bundle-plugins.ts")],
    {
      cwd: root,
      env: { ...process.env, BUNDLE_PLUGINS_DEST: staged },
    },
  );
  expect(bundled.exitCode).toBe(0);
  const selected = selectPluginsByEnabledNames(staged, ["toolu", "ast-grep"]);
  expect(selected.ok).toBe(true);
  if (!selected.ok) return;
  const projectRoot = await mkdtemp(join(tmpBase, "toolu-oc-registry-project-"));
  const configRoot = await mkdtemp(join(tmpBase, "toolu-oc-registry-data-"));
  const bin = await mkdtemp(join(tmpBase, "toolu-oc-registry-bin-"));
  await symlink(Bun.which("git") ?? "/usr/bin/git", join(bin, "git"));
  await symlink(process.execPath, join(bin, "bun"));
  const env = { PATH: bin, TOOLU_HOST_OVERRIDE: "opencode" };
  const boot = await bootstrapRuntime({
    repoRoot: packageRoot,
    projectRoot,
    dataRoot: configRoot,
    plugins: selected.plugins,
    isolatedHome: await mkdtemp(join(tmpBase, "toolu-oc-registry-home-")),
    env,
  });
  expect(boot.status).toBe("ready");
  if (boot.status !== "ready") return;
  expect(boot.artifacts.some((path) => path.endsWith("ast-grep@toolu__search-nudge.js"))).toBe(
    true,
  );
  const handler = createPermissionEvaluateHandler({
    repoRoot: packageRoot,
    configRoot,
    permissionContext: { cwd: projectRoot, projectRoot, worktree: projectRoot },
    env,
  });
  const event: PermissionEvaluationEvent = {
    sessionID: "sess_registry_1",
    action: "bash",
    resources: [],
    metadata: { command: "rg TODO src" },
    effect: "allow",
  };
  await handler(event);
  expect(event.effect).toBe("allow");
  expect(event.message).toContain("grep/rg in Bash detected");
});

test("AC-3: runtime_failure from bad repoRoot maps to deny", async () => {
  const { projectRoot, envPath } = await protectedEnvProject();
  const handler = createPermissionEvaluateHandler({
    repoRoot: join(tmpBase, "toolu-nonexistent-repo-root"),
    configRoot: join(projectRoot, ".opencode", "toolu", "state"),
    permissionContext: {
      cwd: projectRoot,
      projectRoot,
      worktree: projectRoot,
    },
  });

  const event = editEvent(envPath);
  await handler(event);

  expect(event.effect).toBe("deny");
  expect(typeof event.message).toBe("string");
  expect(event.message && event.message.length > 0).toBe(true);
});

test("#343: gates see no other host's root and read the global config from userConfigRoot", () => {
  const poisoned = Object.fromEntries(FOREIGN_HOST_VARS.map((key) => [key, `/poison/${key}`]));
  const base = {
    repoRoot: "/repo",
    configRoot: "/data",
    permissionContext: { cwd: "/p/sub", projectRoot: "/p", worktree: "/p" },
    env: { ...poisoned, KEEP: "1", TOOLU_PROJECT_DIR: "/elsewhere" },
  };
  const opts = { ...base, userConfigRoot: "/global" };
  const env = gateEnv(opts, "/repo/plugins/toolu");
  for (const key of FOREIGN_HOST_VARS) expect(env[key]).toBeUndefined();
  expect(env).toMatchObject({
    KEEP: "1",
    TOOLU_USER_CONFIG_DIR: "/global",
    TOOLU_CONFIG_DIR: "/data",
    TOOLU_PROJECT_DIR: "/p",
    TOOLU_PROJECT_CONFIG_DIRNAME: ".opencode",
    TOOLU_HOST_OVERRIDE: "opencode",
    TOOLU_SETTINGS_DIR: "/repo/plugins/toolu/settings",
  });
  // Without userConfigRoot (the legacy permission.evaluate route) nothing is added.
  expect(gateEnv(base, "/repo/plugins/toolu").TOOLU_USER_CONFIG_DIR).toBe(
    process.env.TOOLU_USER_CONFIG_DIR,
  );
});
