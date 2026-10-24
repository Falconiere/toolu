# Preflight: this session and each worker host

The detail behind the skill's step 1. Stop at the first failure and print its
fix.

## This session

- Run `toolu doctor --host <host>` for the host you run on: `claude`, `codex`,
  `cursor` or `muse`. Name the host, because toolu can't tell every host's
  shell apart. A Cursor Agent shell, for example, looks like Claude Code's.
- `herdr status` and `toolu epic status` must answer. `"engine":"down"` is
  fine, because `toolu epic start` starts the engine.
- `Operation not permitted` from either one means your host's sandbox is
  blocking the engine and herdr sockets and the epic state directory. Every
  later step would fail the same way, so run your commands outside the
  sandbox:
  - **Claude Code:** disable the sandbox for those calls.
  - **Codex:** start it with `--sandbox danger-full-access`, or approve its
    escalation.
  - **Cursor Agent:** start it with `--sandbox disabled`.
  - **Muse Code:** start it with `--disable-sandbox`, or approve its one-time
    unsandboxed run.

## Each worker host in `--hosts`

`--hosts` defaults to `claude`. Run these checks in order for each host:

1. Its CLI is on `PATH`, and it is logged in:

   | Host | CLI | Logged in when | Fix |
   |---|---|---|---|
   | `claude` | `claude` | `claude auth status` shows `"loggedIn": true` | `claude auth login` |
   | `codex` | `codex` | `codex login status` prints `Logged in` | `codex login` |
   | `cursor` | `cursor-agent` | `cursor-agent status` prints `Logged in` | `cursor-agent login`; for `Your macOS login keychain is locked`, `security unlock-keychain` |
   | `muse` | `muse` | `META_API_KEY` is set, or its auth file exists: `MUSE_AUTH_PATH`, else `$XDG_CONFIG_HOME/muse/auth.json`, else `~/.config/muse/auth.json` | `muse login` |

2. `toolu plugins list --host <host>` succeeds, and none of `toolu`,
   `toolu-review`, `pr-babysit`, `brainstorm` or `delivery-flow` is on a
   `not installed` row. The fix is
   `toolu plugins install delivery-flow --host <host>`. If the command itself
   fails, your `toolu` is too old for that host. Check the `binary` line of
   this session's doctor.
3. `toolu --host <host> doctor` prints no `fail` line and no `warn plugins`
   line.
   - A `warn plugins` line means the plugins' hooks won't run, and the line
     names its fix. An unapproved Muse plugin, for example, prints
     `warn plugins: toolu capabilities need approval; muse plugins approve toolu`.
   - Doctor exits 0 on a warning, so read the lines, not the exit code.
   - A `fail skew` line that names only plugins outside those five is a note,
     not a stop. Those are retired plugins, such as `context7`.

Report one line per host: `ready` or `not ready: <fix>`. Launch nothing while
any host in `--hosts` is not ready. Drop a host only if the user agrees.
