/** Isolated clean-install smoke for the three shipped hosts (#279). */
import { spawnSync } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { z } from "zod";
import { claudeAdapter } from "../../tools/toolu-cli/src/host/claude.ts";
import { codexAdapter } from "../../tools/toolu-cli/src/host/codex.ts";
import { stagePlugins } from "../../tools/toolu-opencode/scripts/bundle-plugins.ts";
import { selectPluginsByEnabledNames } from "../../tools/toolu-opencode/src/select/resolve.ts";
import { bootstrapRuntime } from "../../tools/toolu-opencode/src/bootstrap/runtime.ts";
import { createPermissionEvaluateHandler } from "../../tools/toolu-opencode/src/adapter/evaluate.ts";
import type { PermissionEvaluationEvent } from "../../tools/toolu-opencode/src/adapter/permission-map.ts";

const ROOT = resolve(import.meta.dir, "../..");
const HookOutput = z.object({
  hookSpecificOutput: z.object({ permissionDecision: z.string() }).optional(),
});

function run(argv: string[], cwd: string, env: Record<string, string>): string {
  const result = spawnSync(argv[0] ?? "", argv.slice(1), {
    cwd,
    env,
    encoding: "utf8",
    timeout: 120_000,
  });
  if (result.error) throw new Error(`${argv.join(" ")}: ${result.error.message}`);
  if (result.status !== 0)
    throw new Error(
      `${argv.join(" ")} exited ${String(result.status)}: ${result.stdout.trim()} ${result.stderr.trim()}`,
    );
  return result.stdout;
}

function isolatedEnv(root: string): Record<string, string> {
  const home = join(root, "home");
  const claude = join(root, "claude");
  const codex = join(root, "codex");
  const toolu = join(root, "toolu");
  const xdg = join(root, "config");
  const opencode = join(xdg, "opencode");
  for (const dir of [home, claude, codex, toolu, opencode]) mkdirSync(dir, { recursive: true });
  return {
    ...Object.fromEntries(
      Object.entries(process.env).filter(
        (entry): entry is [string, string] => entry[1] !== undefined,
      ),
    ),
    HOME: home,
    CLAUDE_CONFIG_DIR: claude,
    CODEX_HOME: codex,
    TOOLU_CONFIG_DIR: toolu,
    XDG_CONFIG_HOME: xdg,
    OPENCODE_CONFIG_DIR: opencode,
    BUN_INSTALL_CACHE_DIR: join(root, "bun-cache"),
    npm_config_cache: join(root, "npm-cache"),
    TOOLU_BUN: process.execPath,
  };
}

function protectedProject(root: string, host: "claude" | "codex" | "opencode"): string {
  const project = join(root, `${host}-project`);
  mkdirSync(join(project, `.${host}`), { recursive: true });
  writeFileSync(join(project, ".env"), "SECRET=untouched\n");
  writeFileSync(
    join(project, `.${host}`, "toolu.config.json"),
    '{"version":1,"gates":{"protectedFiles":{"mode":"block"}}}\n',
  );
  return project;
}

function smokeCommandHost(
  host: "claude" | "codex",
  root: string,
  env: Record<string, string>,
): void {
  const project = protectedProject(root, host);
  const bin = host;
  run([bin, "plugin", "marketplace", "add", ROOT], project, env);
  run(
    host === "claude"
      ? [bin, "plugin", "install", "toolu@toolu", "--scope", "user", "-y"]
      : [bin, "plugin", "add", "toolu@toolu"],
    project,
    env,
  );
  const listed = run([bin, "plugin", "list", "--json"], project, env);
  const installed = (host === "claude" ? claudeAdapter : codexAdapter)
    .parseList(listed)
    .find((entry) => entry.name === "toolu");
  if (!installed?.path) throw new Error(`${host}: toolu install path missing from plugin list`);
  const pluginRoot = installed.path;
  const file = join(project, ".env");
  const payload = JSON.stringify({
    tool_name: "Edit",
    tool_input: { file_path: file },
    session_id: "clean-install-smoke",
    tool_use_id: "edit-1",
    cwd: project,
  });
  const hookEnv = {
    ...env,
    CLAUDE_PLUGIN_ROOT: pluginRoot,
    PLUGIN_ROOT: pluginRoot,
    CLAUDE_PROJECT_DIR: project,
    TOOLU_HOST_OVERRIDE: host,
    TOOLU_SETTINGS_DIR: join(pluginRoot, "settings"),
  };
  const hook = spawnSync(process.execPath, [join(pluginRoot, "hooks/dist/pre-tools.js")], {
    cwd: project,
    env: hookEnv,
    input: payload,
    encoding: "utf8",
    timeout: 30_000,
  });
  if (hook.error || hook.status !== 0)
    throw new Error(`${host}: installed hook failed: ${hook.error?.message ?? hook.stderr}`);
  const decision = HookOutput.parse(JSON.parse(hook.stdout)).hookSpecificOutput?.permissionDecision;
  if (decision !== "deny") throw new Error(`${host}: protected edit decision was ${decision}`);
  if (readFileSync(file, "utf8") !== "SECRET=untouched\n")
    throw new Error(`${host}: protected file changed`);
  process.stdout.write(`${host}: installed toolu bundle denied protected edit (temp config)\n`);
}

function stageOpenCodePackage(root: string): string {
  const packageRoot = join(root, "opencode-package");
  mkdirSync(packageRoot, { recursive: true });
  for (const name of ["package.json", "README.md", "LICENSE", "src", "generated"]) {
    cpSync(join(ROOT, "tools/toolu-opencode", name), join(packageRoot, name), {
      recursive: true,
    });
  }
  // The temp package is already staged below; the source prepack script expects
  // the repository layout and would otherwise run again from this copied root.
  const loaded: unknown = JSON.parse(readFileSync(join(packageRoot, "package.json"), "utf8"));
  const manifest = z
    .object({ scripts: z.record(z.string(), z.unknown()).optional() })
    .passthrough()
    .parse(loaded);
  manifest.scripts = {};
  writeFileSync(join(packageRoot, "package.json"), `${JSON.stringify(manifest)}\n`);
  stagePlugins(join(ROOT, "plugins"), join(packageRoot, "plugins"));
  return packageRoot;
}

function installOpenCodePlugin(
  root: string,
  project: string,
  packageRoot: string,
  env: Record<string, string>,
): void {
  mkdirSync(join(project, ".opencode/toolu"), { recursive: true });
  writeFileSync(
    join(project, ".opencode/toolu/plugins.json"),
    '{"version":1,"enabled":["toolu"]}\n',
  );
  writeFileSync(
    join(project, "package.json"),
    '{"private":true,"type":"module","trustedDependencies":["@opencode/cli"]}\n',
  );
  run([process.execPath, "add", "@opencode/cli@2.0.12"], project, env);
  const cli = [join(project, "node_modules/.bin/opencode")];
  const version = run([...cli, "--version"], project, env).trim();
  if (version !== "opencode v2.0.12") throw new Error(`opencode: expected v2.0.12, got ${version}`);
  run(["git", "init", "-q"], packageRoot, env);
  run(["git", "config", "user.name", "toolu smoke"], packageRoot, env);
  run(["git", "config", "user.email", "smoke@toolu.test"], packageRoot, env);
  run(["git", "add", "-A"], packageRoot, env);
  run(["git", "commit", "-qm", "fixture: staged OpenCode plugin"], packageRoot, env);
  const spec = `git+file://${packageRoot}`;
  run([...cli, "plugin", "add", spec], project, env);
  const config = readFileSync(join(root, "config/opencode/opencode.json"), "utf8");
  if (!config.includes(spec)) throw new Error("opencode: installed plugin absent from temp config");
}

async function smokeOpenCode(root: string, env: Record<string, string>): Promise<void> {
  const project = protectedProject(root, "opencode");
  const packageRoot = stageOpenCodePackage(root);
  installOpenCodePlugin(root, project, packageRoot, env);
  const selected = selectPluginsByEnabledNames(join(packageRoot, "plugins"), ["toolu"]);
  if (!selected.ok) throw new Error(`opencode selection: ${selected.reason}`);
  const boot = await bootstrapRuntime({
    repoRoot: packageRoot,
    projectRoot: project,
    dataRoot: join(root, "toolu"),
    plugins: selected.plugins,
    isolatedHome: join(root, "home"),
    env,
  });
  if (boot.status !== "ready") throw new Error(`opencode bootstrap: ${boot.reason}`);
  const file = join(project, ".env");
  const handler = createPermissionEvaluateHandler({
    repoRoot: packageRoot,
    configRoot: join(root, "toolu"),
    permissionContext: { cwd: project, projectRoot: project, worktree: project },
    env: {
      ...env,
      TOOLU_SETTINGS_DIR: join(packageRoot, "plugins/toolu/settings"),
      TOOLU_HOST_OVERRIDE: "opencode",
      TOOLU_PROJECT_CONFIG_DIRNAME: ".opencode",
    },
  });
  const event: PermissionEvaluationEvent = {
    sessionID: "clean-install-smoke",
    action: "edit",
    resources: [file],
    effect: "allow",
    metadata: { toolCallId: "edit-1" },
  };
  await handler(event);
  if (event.effect !== "deny") throw new Error(`opencode: protected edit was ${event.effect}`);
  if (readFileSync(file, "utf8") !== "SECRET=untouched\n")
    throw new Error("opencode: protected file changed");
  process.stdout.write(
    "opencode v2.0.12: local Git plugin installed, bootstrapped, and denied protected edit (temp config)\n",
  );
}

async function main(): Promise<void> {
  const root = mkdtempSync(join(tmpdir(), "toolu-clean-install-"));
  try {
    const env = isolatedEnv(root);
    smokeCommandHost("claude", root, env);
    smokeCommandHost("codex", root, env);
    await smokeOpenCode(root, env);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

await main();
