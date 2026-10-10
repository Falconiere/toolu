/** OpenCode evaluate integration over shared permission and registry scenarios. */
import { expect, test } from "bun:test";
import { mkdir, mkdtemp, symlink, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { builtTooluBinary } from "@toolu/conformance/harness/entry-command";
import { readCaseFile } from "@toolu/conformance/harness/json-cases";
import { z } from "zod";
import { FOREIGN_HOST_VARS } from "../../host/runtime-env.ts";
import { bootstrapRuntime } from "../../bootstrap/runtime.ts";
import { selectPluginsByEnabledNames } from "../../select/resolve.ts";
import { createGateDecider, createPermissionEvaluateHandler, gateEnv } from "../evaluate.ts";
import type { PermissionEvaluationEvent } from "../permission-map.ts";

const tmpBase = process.env.TMPDIR ?? "/tmp";
const root = join(dirname(fileURLToPath(import.meta.url)), "../../../../..");
const EffectSchema = z.enum(["allow", "ask", "deny"]);
const EventSchema = z.strictObject({
  sessionID: z.string(),
  action: z.string(),
  effect: EffectSchema,
  toolCallId: z.string().optional(),
});
const ProtectedSchema = z.strictObject({
  name: z.string(),
  kind: z.literal("evaluate-protected"),
  config: z.json(),
  file: z.string(),
  body: z.string(),
  pathTools: z.array(z.enum(["git", "bun"])).optional(),
  event: EventSchema,
  expected: z.strictObject({ effect: EffectSchema, messageContains: z.string().optional() }),
});
const BadRootSchema = z.strictObject({
  name: z.string(),
  kind: z.literal("evaluate-bad-root"),
  config: z.json(),
  file: z.string(),
  body: z.string(),
  badRoot: z.string(),
  event: EventSchema,
  expected: z.strictObject({ effect: EffectSchema, messageNonempty: z.boolean() }),
});
const RegistrySchema = z.strictObject({
  name: z.string(),
  kind: z.literal("evaluate-registry"),
  plugins: z.array(z.string()),
  pathTools: z.array(z.enum(["git", "bun"])),
  command: z.string(),
  artifactSuffix: z.string(),
  advisoryContains: z.string(),
  event: EventSchema,
  expected: z.strictObject({ effect: EffectSchema }),
});
const GateEnvSchema = z.strictObject({
  name: z.string(),
  kind: z.literal("evaluate-gate-env"),
  base: z.strictObject({
    repoRoot: z.string(),
    configRoot: z.string(),
    cwd: z.string(),
    projectRoot: z.string(),
    worktree: z.string(),
    env: z.record(z.string(), z.string()),
    userConfigRoot: z.string(),
    pluginRoot: z.string(),
  }),
  expected: z.record(z.string(), z.string()),
});
const KindSchema = z.enum([
  "map",
  "decision",
  "evaluate-protected",
  "evaluate-bad-root",
  "evaluate-registry",
  "evaluate-gate-env",
]);
const cases = readCaseFile(
  resolve(import.meta.dir, "../../../../../fixtures/opencode/permission-evaluate.json"),
);

async function protectedProject(
  file: string,
  body: string,
  config: z.infer<typeof ProtectedSchema>["config"],
): Promise<string> {
  const projectRoot = await mkdtemp(join(tmpBase, "toolu-oc-eval-"));
  await writeFile(join(projectRoot, file), body, "utf8");
  await mkdir(join(projectRoot, ".opencode"), { recursive: true });
  await writeFile(join(projectRoot, ".opencode/toolu.config.json"), JSON.stringify(config), "utf8");
  return projectRoot;
}

async function toolPath(names: readonly ("git" | "bun")[]): Promise<string> {
  const bin = await mkdtemp(join(tmpBase, "toolu-oc-bin-"));
  await Promise.all(
    names.map((name) => {
      const target =
        name === "git"
          ? (Bun.which("git") ?? "/usr/bin/git")
          : (Bun.which("bun") ?? process.execPath);
      return symlink(target, join(bin, name));
    }),
  );
  return bin;
}

function evalEvent(
  input: z.infer<typeof EventSchema>,
  resources: string[],
  command?: string,
): PermissionEvaluationEvent {
  return {
    sessionID: input.sessionID,
    action: input.action,
    resources,
    effect: input.effect,
    metadata: {
      ...(input.toolCallId === undefined ? {} : { toolCallId: input.toolCallId }),
      ...(command === undefined ? {} : { command }),
    },
  };
}

for (const raw of cases) {
  const kind = KindSchema.parse(raw.kind);
  if (kind === "evaluate-protected" || kind === "evaluate-bad-root") {
    const c =
      raw.kind === "evaluate-protected" ? ProtectedSchema.parse(raw) : BadRootSchema.parse(raw);
    test(c.name, async () => {
      const projectRoot = await protectedProject(c.file, c.body, c.config);
      const bin =
        c.kind === "evaluate-protected" && c.pathTools !== undefined
          ? await toolPath(c.pathTools)
          : undefined;
      const handler = createPermissionEvaluateHandler({
        repoRoot: c.kind === "evaluate-bad-root" ? join(tmpBase, c.badRoot) : root,
        configRoot: join(projectRoot, ".opencode", "toolu", "state"),
        permissionContext: { cwd: projectRoot, projectRoot, worktree: projectRoot },
        ...(c.kind === "evaluate-bad-root"
          ? {}
          : {
              env: {
                ...(bin === undefined ? {} : { PATH: bin }),
                TOOLU_SETTINGS_DIR: join(root, "plugins/toolu/settings"),
                TOOLU_HOST_OVERRIDE: "opencode",
                TOOLU_PROJECT_CONFIG_DIRNAME: ".opencode",
              },
            }),
      });
      const event = evalEvent(c.event, [join(projectRoot, c.file)]);
      await handler(event);
      expect(event.effect).toBe(c.expected.effect);
      if (c.kind === "evaluate-protected" && c.expected.messageContains !== undefined)
        expect(event.message).toContain(c.expected.messageContains);
      if (c.kind === "evaluate-bad-root" && c.expected.messageNonempty)
        expect(event.message?.length).toBeGreaterThan(0);
    });
  } else if (kind === "evaluate-registry") {
    const c = RegistrySchema.parse(raw);
    test(c.name, async () => {
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
      const selected = selectPluginsByEnabledNames(staged, c.plugins);
      expect(selected.ok).toBe(true);
      if (!selected.ok) return;
      const projectRoot = await mkdtemp(join(tmpBase, "toolu-oc-registry-project-"));
      const configRoot = await mkdtemp(join(tmpBase, "toolu-oc-registry-data-"));
      const env = {
        PATH: await toolPath(c.pathTools),
        TOOLU_HOST_OVERRIDE: "opencode",
        TOOLU_BIN: builtTooluBinary() ?? join(root, "target/debug/toolu"),
      };
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
      expect(boot.artifacts.some((path) => path.endsWith(c.artifactSuffix))).toBe(true);
      const options = {
        repoRoot: packageRoot,
        configRoot,
        permissionContext: { cwd: projectRoot, projectRoot, worktree: projectRoot },
        env,
      };
      const decider = createGateDecider(options);
      if (!decider.ok) throw new Error(decider.reason);
      const decision = await decider.decide({
        session_id: c.event.sessionID,
        tool_use_id: "call_registry_1",
        cwd: projectRoot,
        tool_name: "Bash",
        tool_input: { command: c.command },
      });
      expect(decision.kind).toBe("advisory");
      if (decision.kind === "advisory") expect(decision.message).toContain(c.advisoryContains);
      const event = evalEvent(c.event, [], c.command);
      await createPermissionEvaluateHandler(options)(event);
      expect(event.effect).toBe(c.expected.effect);
      expect(event.message).toBeUndefined();
    });
  } else if (kind === "evaluate-gate-env") {
    const c = GateEnvSchema.parse(raw);
    test(c.name, () => {
      const poisoned = Object.fromEntries(FOREIGN_HOST_VARS.map((key) => [key, `/poison/${key}`]));
      const base = {
        repoRoot: c.base.repoRoot,
        configRoot: c.base.configRoot,
        permissionContext: {
          cwd: c.base.cwd,
          projectRoot: c.base.projectRoot,
          worktree: c.base.worktree,
        },
        env: { ...poisoned, ...c.base.env },
      };
      const env = gateEnv({ ...base, userConfigRoot: c.base.userConfigRoot }, c.base.pluginRoot);
      for (const key of FOREIGN_HOST_VARS) expect(env[key]).toBeUndefined();
      expect(env).toMatchObject(c.expected);
      expect(gateEnv(base, c.base.pluginRoot).TOOLU_USER_CONFIG_DIR).toBe(
        process.env.TOOLU_USER_CONFIG_DIR,
      );
    });
  }
}
