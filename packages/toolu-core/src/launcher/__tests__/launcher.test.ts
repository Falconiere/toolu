import { afterAll, describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  isEnforcingEvent,
  launcherCommand,
  launcherCommandWindows,
  launcherHook,
  missingRuntimeMessage,
  runtimeDiagnostic,
} from "../launcher.ts";

const BUN = process.execPath;
const BARE_PATH = "/usr/bin:/bin";
const temps: string[] = [];

afterAll(() => {
  for (const dir of temps) rmSync(dir, { recursive: true, force: true });
});

function temp(prefix: string): string {
  const dir = mkdtempSync(join(tmpdir(), prefix));
  temps.push(dir);
  return dir;
}

/** A plugin root (with a space in its path) holding a real bundle that echoes stdin and exits with `input.exit`. */
function probePlugin(): string {
  const root = join(temp("launcher-"), "plugin root");
  mkdirSync(join(root, "hooks/dist"), { recursive: true });
  writeFileSync(
    join(root, "hooks/dist/probe.js"),
    "const input = await Bun.stdin.text();\nprocess.stdout.write(`got:${input}`);\nprocess.exit(JSON.parse(input).exit);\n",
  );
  return root;
}

/** A HOME with no ~/.bun, and optionally one whose ~/.bun/bin/bun is the real binary. */
function home(withBun: boolean): string {
  const dir = temp("launcher-home-");
  if (withBun) {
    mkdirSync(join(dir, ".bun/bin"), { recursive: true });
    symlinkSync(BUN, join(dir, ".bun/bin/bun"));
  }
  return dir;
}

/** A directory whose only entry is `bun`, symlinked to the real binary. */
function pathWithBun(): string {
  const dir = temp("launcher-bin-");
  symlinkSync(BUN, join(dir, "bun"));
  return `${dir}:${BARE_PATH}`;
}

interface Run {
  status: number | null;
  stdout: string;
  stderr: string;
}

function run(shell: string, command: string, env: Record<string, string>, input = ""): Run {
  const result = spawnSync(shell, ["-c", command], { env, input, encoding: "utf8" });
  if (result.error) throw result.error;
  return { status: result.status, stdout: result.stdout, stderr: result.stderr };
}

const target = (event: string) => ({ plugin: "demo", event, entry: "probe" });

// sh and bash exist on every supported host; dash and zsh are required on macOS
// and run on Linux when installed, reported as a named skip otherwise.
const SHELLS = ["sh", "bash", "dash", "zsh"].map((name) => ({ name, path: Bun.which(name) }));
const required = new Set(
  process.platform === "darwin" ? ["sh", "bash", "dash", "zsh"] : ["sh", "bash"],
);

test("every required shell is installed", () => {
  for (const shell of SHELLS) if (required.has(shell.name)) expect(shell.path).not.toBeNull();
});

test("Bun is really absent from the bare PATH used for missing-runtime cases", () => {
  expect(existsSync("/usr/bin/bun") || existsSync("/bin/bun")).toBe(false);
});

for (const shell of SHELLS) {
  describe.skipIf(shell.path === null)(`launcher under ${shell.name}`, () => {
    const sh = shell.path ?? "";

    for (const event of ["PreToolUse", "PermissionRequest"]) {
      test(`${event} blocks with one stderr line when Bun is absent`, () => {
        const root = probePlugin();
        const out = run(sh, launcherCommand(target(event)), {
          PATH: BARE_PATH,
          HOME: home(false),
          CLAUDE_PLUGIN_ROOT: root,
        });
        expect(out.status).toBe(2);
        expect(out.stdout).toBe("");
        expect(out.stderr).toBe(`blocked: ${missingRuntimeMessage("demo")}\n`);
      });
    }

    for (const event of ["SessionStart", "UserPromptSubmit", "PostToolUse"]) {
      test(`${event} prints the advisory payload and exits 0 when Bun is absent`, () => {
        const out = run(sh, launcherCommand(target(event)), {
          PATH: BARE_PATH,
          HOME: home(false),
          CLAUDE_PLUGIN_ROOT: probePlugin(),
        });
        expect(out.status).toBe(0);
        expect(out.stderr).toBe("");
        expect(JSON.parse(out.stdout)).toEqual({ systemMessage: missingRuntimeMessage("demo") });
      });
    }

    test("TOOLU_BUN runs the bundle with stdin intact and propagates exit 2", () => {
      const out = run(
        sh,
        launcherCommand(target("PreToolUse")),
        { PATH: BARE_PATH, HOME: home(false), CLAUDE_PLUGIN_ROOT: probePlugin(), TOOLU_BUN: BUN },
        '{"exit":2}',
      );
      expect(out.status).toBe(2);
      expect(out.stdout).toBe('got:{"exit":2}');
    });

    test("bun on PATH runs the bundle and exits 0", () => {
      const out = run(
        sh,
        launcherCommand(target("SessionStart")),
        { PATH: pathWithBun(), HOME: home(false), CLAUDE_PLUGIN_ROOT: probePlugin() },
        '{"exit":0}',
      );
      expect(out.status).toBe(0);
      expect(out.stdout).toBe('got:{"exit":0}');
    });

    test("~/.bun/bin/bun is the last resort", () => {
      const out = run(
        sh,
        launcherCommand(target("PreToolUse")),
        { PATH: BARE_PATH, HOME: home(true), CLAUDE_PLUGIN_ROOT: probePlugin() },
        '{"exit":0}',
      );
      expect(out.status).toBe(0);
      expect(out.stdout).toBe('got:{"exit":0}');
    });

    test("a non-executable TOOLU_BUN falls through to PATH", () => {
      const fake = join(temp("launcher-fake-"), "bun");
      writeFileSync(fake, "not a binary");
      chmodSync(fake, 0o644);
      const out = run(
        sh,
        launcherCommand(target("PreToolUse")),
        {
          PATH: pathWithBun(),
          HOME: home(false),
          CLAUDE_PLUGIN_ROOT: probePlugin(),
          TOOLU_BUN: fake,
        },
        '{"exit":0}',
      );
      expect(out.status).toBe(0);
      expect(out.stdout).toBe('got:{"exit":0}');
    });

    test("textual ${CLAUDE_PLUGIN_ROOT} substitution (Claude Code) works for a path with spaces", () => {
      const root = probePlugin();
      const command = launcherCommand(target("PreToolUse")).replaceAll(
        "${CLAUDE_PLUGIN_ROOT}",
        root,
      );
      const out = run(
        sh,
        command,
        { PATH: BARE_PATH, HOME: home(false), TOOLU_BUN: BUN },
        '{"exit":0}',
      );
      expect(out.status).toBe(0);
      expect(out.stdout).toBe('got:{"exit":0}');
    });
  });
}

test("only PreToolUse and PermissionRequest are enforcing", () => {
  expect(isEnforcingEvent("PreToolUse")).toBe(true);
  expect(isEnforcingEvent("PermissionRequest")).toBe(true);
  for (const event of ["SessionStart", "UserPromptSubmit", "PostToolUse", "Stop", "PreCompact"]) {
    expect(isEnforcingEvent(event)).toBe(false);
  }
});

test("the command carries no ${...} other than the literal plugin-root token", () => {
  const command = launcherCommand(target("PreToolUse"));
  expect(command.match(/\$\{[^}]*\}/g)).toEqual(["${CLAUDE_PLUGIN_ROOT}"]);
});

test("the missing-runtime message is safe inside sh single quotes, cmd echo and JSON", () => {
  expect(missingRuntimeMessage("demo")).toMatch(/^demo plugin: Bun runtime not found/);
  expect(missingRuntimeMessage("demo")).not.toMatch(/[%&|<>^()'"\\]/);
});

test("the Windows variant resolves in the same order and fails the same way", () => {
  const enforcing = launcherCommandWindows(target("PreToolUse"));
  const [toolu, path, home] = [
    "%TOOLU_BUN%",
    "where /q bun",
    "%USERPROFILE%\\.bun\\bin\\bun.exe",
  ].map((s) => enforcing.indexOf(s));
  expect(toolu).toBeGreaterThanOrEqual(0);
  expect(path).toBeGreaterThan(toolu ?? -1);
  expect(home).toBeGreaterThan(path ?? -1);
  expect(enforcing).toContain('"%PLUGIN_ROOT%\\hooks\\dist\\probe.js"');
  expect(enforcing).toContain("exit /b 2");
  expect(enforcing).not.toMatch(/&&|\|\|/);
  const advisory = launcherCommandWindows(target("SessionStart"));
  expect(advisory).toContain(`echo {"systemMessage":"${missingRuntimeMessage("demo")}"}`);
  expect(advisory).not.toContain("exit /b 2");
});

test("launcherHook pairs both strings on a command hook", () => {
  expect(launcherHook(target("SessionStart"))).toEqual({
    type: "command",
    command: launcherCommand(target("SessionStart")),
    commandWindows: launcherCommandWindows(target("SessionStart")),
  });
});

test("invalid entry names are rejected", () => {
  for (const entry of ["../x", "Probe", "a b", "", "x.js"]) {
    expect(() => launcherCommand({ plugin: "demo", event: "PreToolUse", entry })).toThrow();
  }
});

test("runtimeDiagnostic names the version and path", () => {
  expect(runtimeDiagnostic("/opt/bun", "1.4.2")).toEqual({
    systemMessage: "toolu runtime: bun 1.4.2 at /opt/bun",
  });
});
