/**
 * Probe: a hook `ask` survives a blanket permissions allowlist.
 *
 * The relaxed denylist is only useful if `permissionDecision: "ask"` still
 * reaches the user after toolu writes `Bash(*)` into settings.local.json.
 * Claude Code's changelog settles the rule — `permissions.deny` overrides a
 * hook's ask, and nothing says `permissions.allow` does — but a rule you have
 * not exercised is a rule you are guessing at. This runs the real PreToolUse
 * pipeline against a real command in a repo that HAS the blanket allowlist on
 * disk and asserts an ask comes out. The host's half (does the CLI render the
 * prompt) can only be seen by a human running the CLI.
 *
 * Usage: bun run tooling/src/probe-ask.ts
 * Exit 0 = ask emitted; exit 1 = something else came out (details on stderr).
 */
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { launcherCommand } from "@toolu/core/launcher";

const PLUGIN = resolve(import.meta.dir, "../../plugins/toolu");

/** The PreToolUse command hooks.json runs: the dispatcher bundle behind its launcher. */
const HOOK = launcherCommand({ plugin: "toolu", event: "PreToolUse", entry: "pre-tools" });

function git(cwd: string, ...args: string[]): void {
  const res = spawnSync("git", args, { cwd, encoding: "utf8" });
  if (res.status !== 0) throw new Error(`git ${args.join(" ")}: ${res.stderr.trim()}`);
}

/** A repo with the blanket allowlist, a one-rule denylist, and bashCommands pinned to ask. */
function setUp(tmp: string): { repo: string; home: string; settings: string } {
  const repo = join(tmp, "repo");
  const home = join(tmp, "home");
  const settings = join(tmp, "settings");
  for (const dir of [join(repo, ".claude"), join(home, ".claude"), settings])
    mkdirSync(dir, { recursive: true });
  git(repo, "init", "-q", "-b", "main");
  git(repo, "config", "user.email", "probe@example.com");
  git(repo, "config", "user.name", "probe");
  git(repo, "commit", "-q", "--allow-empty", "-m", "init");
  writeFileSync(
    join(repo, ".claude/settings.local.json"),
    '{"permissions":{"allow":["Bash(*)","Edit","Write"]}}\n',
  );
  writeFileSync(join(settings, "bash-denylist.txt"), "node -e\n");
  writeFileSync(join(settings, "bash-allowlist.txt"), "");
  writeFileSync(
    join(repo, ".claude/toolu.config.json"),
    '{"version":1,"gates":{"bashCommands":{"mode":"ask"}}}',
  );
  return { repo, home, settings };
}

function decisionOf(output: string): string {
  try {
    const doc: unknown = JSON.parse(output);
    const specific: unknown =
      typeof doc === "object" && doc !== null ? Reflect.get(doc, "hookSpecificOutput") : null;
    const decision: unknown =
      typeof specific === "object" && specific !== null
        ? Reflect.get(specific, "permissionDecision")
        : null;
    return typeof decision === "string" ? decision : "none";
  } catch {
    return "unparseable";
  }
}

function main(): number {
  const tmp = realpathSync(mkdtempSync(join(tmpdir(), "probe-ask-")));
  try {
    const { repo, home, settings } = setUp(tmp);
    const payload = JSON.stringify({
      tool_name: "Bash",
      tool_input: { command: 'node -e "console.log(1)"' },
    });
    const res = spawnSync("/bin/sh", ["-c", HOOK], {
      cwd: repo,
      input: payload,
      encoding: "utf8",
      env: {
        PATH: process.env["PATH"] ?? "",
        HOME: home,
        CLAUDE_PLUGIN_ROOT: PLUGIN,
        TOOLU_BUN: process.execPath,
        CLAUDE_PROJECT_DIR: repo,
        TOOLU_PROJECT_DIR: repo,
        TOOLU_SETTINGS_DIR: settings,
      },
    });
    const decision = decisionOf(res.stdout);
    if (decision === "ask") {
      process.stdout.write(
        "probe-ask: ask emitted with Bash(*) allowlisted — the gate can still prompt.\n",
      );
      return 0;
    }
    console.error(`probe-ask: expected an ask decision, got "${decision}".`);
    console.error(`probe-ask: raw hook output was: ${res.stdout}`);
    return 1;
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}

if (import.meta.main) process.exitCode = main();
