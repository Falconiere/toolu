/**
 * The hooks.json launcher contract (#250): the only source of the command
 * strings that start a plugin's committed Bun bundle. `tooling/src/check-hooks-json.ts`
 * fails CI when a hooks.json launcher entry differs from what this module emits.
 *
 * POSIX `command` resolves Bun as `TOOLU_BUN`, then `PATH`, then `~/.bun/bin/bun`
 * (docs/runtime.md) and `exec`s `hooks/dist/<entry>.js`, so stdin, stdout and the
 * bundle's exit code pass through untouched. With no Bun, enforcing events print
 * one stderr line and exit 2 (the host blocks); every other event prints a
 * `systemMessage` payload and exits 0.
 *
 * `${CLAUDE_PLUGIN_ROOT}` is the only braced variable: Claude Code substitutes it
 * as text (and does not always export it), Codex and Cursor export it. Every other
 * variable stays brace-less so no host templater rewrites it.
 *
 * `commandWindows` is Codex's cmd.exe override. It is shape-only: no Windows host
 * runs it in CI (epic #247 non-goal).
 */

/** Events whose hook can prevent the action; a missing runtime must block them. */
export const ENFORCING_EVENTS: ReadonlySet<string> = new Set(["PreToolUse", "PermissionRequest"]);

export interface LauncherTarget {
  /** Plugin directory name, named in the missing-runtime message. */
  readonly plugin: string;
  /** Host event the hook is registered under. */
  readonly event: string;
  /** Bundle name: `hooks/dist/<entry>.js`. */
  readonly entry: string;
}

export interface LauncherHook {
  readonly type: "command";
  readonly command: string;
  readonly commandWindows: string;
}

const ENTRY = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
const PLUGIN = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;

export function isEnforcingEvent(event: string): boolean {
  return ENFORCING_EVENTS.has(event);
}

/** One line, free of characters that sh single quotes, cmd `echo` or JSON would reinterpret. */
export function missingRuntimeMessage(plugin: string): string {
  return (
    `${plugin} plugin: Bun runtime not found, checked TOOLU_BUN, PATH and ~/.bun/bin/bun. ` +
    "Install Bun 1.4.x from https://bun.sh and restart the session. See docs/runtime.md."
  );
}

/** The found-runtime SessionStart report: which Bun runs this plugin's hooks. */
export function runtimeDiagnostic(bunPath: string, bunVersion: string): { systemMessage: string } {
  return { systemMessage: `toolu runtime: bun ${bunVersion} at ${bunPath}` };
}

function validate(target: LauncherTarget): void {
  if (!ENTRY.test(target.entry)) {
    throw new Error(`launcher entry must match ${ENTRY.source}: ${JSON.stringify(target.entry)}`);
  }
  if (!PLUGIN.test(target.plugin)) {
    throw new Error(
      `launcher plugin must match ${PLUGIN.source}: ${JSON.stringify(target.plugin)}`,
    );
  }
}

function advisoryJson(plugin: string): string {
  return JSON.stringify({ systemMessage: missingRuntimeMessage(plugin) });
}

export function launcherCommand(target: LauncherTarget): string {
  validate(target);
  const bundle = `"\${CLAUDE_PLUGIN_ROOT}/hooks/dist/${target.entry}.js"`;
  const missing = isEnforcingEvent(target.event)
    ? `printf '%s\\n' 'blocked: ${missingRuntimeMessage(target.plugin)}' >&2; exit 2`
    : `printf '%s\\n' '${advisoryJson(target.plugin)}'; exit 0`;
  return [
    "b=",
    'for c in "$TOOLU_BUN" "$(command -v bun 2>/dev/null)" "$HOME/.bun/bin/bun"; do if [ -n "$c" ] && [ -f "$c" ] && [ -x "$c" ]; then b=$c; break; fi; done',
    `if [ -n "$b" ]; then exec "$b" ${bundle}; fi`,
    missing,
  ].join("; ");
}

export function launcherCommandWindows(target: LauncherTarget): string {
  validate(target);
  const bundle = `"%PLUGIN_ROOT%\\hooks\\dist\\${target.entry}.js"`;
  const home = '"%USERPROFILE%\\.bun\\bin\\bun.exe"';
  const missing = isEnforcingEvent(target.event)
    ? `(1>&2 echo blocked: ${missingRuntimeMessage(target.plugin)}& exit /b 2)`
    : `(echo ${advisoryJson(target.plugin)})`;
  return (
    `if exist "%TOOLU_BUN%" ("%TOOLU_BUN%" ${bundle}) else ` +
    `(where /q bun& if not errorlevel 1 (bun ${bundle}) else ` +
    `if exist ${home} (${home} ${bundle}) else ${missing})`
  );
}

export function launcherHook(target: LauncherTarget): LauncherHook {
  return {
    type: "command",
    command: launcherCommand(target),
    commandWindows: launcherCommandWindows(target),
  };
}
