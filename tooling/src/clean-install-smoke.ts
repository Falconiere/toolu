/** Isolated clean-install smoke for the shipped hosts (#279). */
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { z } from "zod";

const ROOT = resolve(import.meta.dir, "../..");
/** `claude plugin list --json`: `[{ id: "<name>@<marketplace>", installPath }]`. */
const ClaudeList = z.array(z.looseObject({ id: z.string(), installPath: z.string().optional() }));
/** `codex plugin list --json`: `{ installed: [{ name, source: { path } }] }`. */
const CodexList = z.looseObject({
  installed: z.array(
    z.looseObject({ name: z.string(), source: z.looseObject({ path: z.string().optional() }).optional() }),
  ),
});

/** Where the host installed plugin `name`, read from its `plugin list --json`. */
function installPath(host: "claude" | "codex", listed: string, name: string): string | undefined {
  const raw: unknown = JSON.parse(listed);
  if (host === "claude") {
    return ClaudeList.parse(raw).find((entry) => entry.id.split("@")[0] === name)?.installPath;
  }
  return CodexList.parse(raw).installed.find((entry) => entry.name === name)?.source?.path;
}

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
  for (const dir of [home, claude, codex, toolu, xdg]) mkdirSync(dir, { recursive: true });
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
    BUN_INSTALL_CACHE_DIR: join(root, "bun-cache"),
    npm_config_cache: join(root, "npm-cache"),
    TOOLU_BUN: process.execPath,
  };
}

function protectedProject(root: string, host: "claude" | "codex"): string {
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
  const pluginRoot = installPath(host, listed, "toolu");
  if (pluginRoot === undefined) throw new Error(`${host}: toolu install path missing from plugin list`);
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

function main(): void {
  const root = mkdtempSync(join(tmpdir(), "toolu-clean-install-"));
  try {
    const env = isolatedEnv(root);
    smokeCommandHost("claude", root, env);
    smokeCommandHost("codex", root, env);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

main();
