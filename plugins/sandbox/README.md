# sandbox

A Linux sandbox for builds, gates and agent CLIs: a Lima VM on macOS that mounts only the project, this machine on Linux. `toolu sandbox setup` provisions the toolchain, and `toolu sandbox run` runs any command there at the same path. Standalone, no dependencies.

## Install

```
/plugin install sandbox@toolu
```

**Prerequisite:** the native [`toolu` binary](../../docs/install.md). Bun is not used or needed. See [docs/runtime.md](../../docs/runtime.md).

For Codex: `codex plugin add sandbox@toolu`, or `toolu plugins install sandbox --host codex`.

This plugin has no hook. In skills, commands and agents, write plain `toolu sandbox <verb>`; never put a fixed binary path in static Markdown. With no SessionStart hook of its own, it relies on plain `toolu` resolving in the agent command shell, or on another installed toolu plugin's SessionStart supplying the native binary's absolute path for that session. See [docs/install.md](../../docs/install.md#agent-command-shell).

Standalone, no dependencies.

## Crate

`crates/sandbox` is this plugin's Rust library. It contributes the `toolu sandbox` namespace; the generated reference is in [docs/cli/](../../docs/cli/README.md).

The backend decides where commands run. `host` runs them on this machine and is the default on Linux, where the machine already is a stock Linux kernel with Landlock and no macOS policy. `lima` runs them in a Lima VM (Debian 13, Lima 2 or newer) that mounts only the project's worktree, its git common dir and `sandbox.mounts`, each writable at its host path, and is the default on macOS. Inside the VM, `CARGO_TARGET_DIR` and `TMPDIR` default to the VM's own disk, because the shared mount is slow for many small files. A stopped VM boots on the next `run`; a running one that lacks this project's mounts needs `toolu sandbox start --restart`, which stops every command running in it. The generated Lima file, the logs of `setup` and the lock live under `~/.local/state/toolu/sandbox/`. The settings are the `sandbox` section of `toolu.config.json` ([docs/config.md](../../docs/config.md#vm-sandbox-sandbox)); `backend`, `name`, `mounts` and `template` are read from your user config only, so a cloned repository cannot choose them.

## What it provides

- `toolu sandbox run [--clone] [--] <command>...` replaces toolu with the command, in the sandbox at this directory's path. The command keeps the terminal, its stdin, stdout and stderr, and its exit status is toolu's. `--clone` runs it in a clone of this checkout on the sandbox's own disk instead, with the staged, unstaged and untracked changes and the remote-tracking refs; ignored build output stays in the clone between runs, and nothing flows back.
- `toolu sandbox shell` opens a login shell there.
- `toolu sandbox setup` creates the VM and provisions the toolchain the gate needs: the project's pinned Rust toolchain, cargo-deny, cargo-machete, cargo-llvm-cov and cargo-nextest at the release pins, Node 22, ast-grep, bubblewrap (Codex and Muse sandbox with it), mold and a static `toolu`. It is idempotent and takes a few minutes the first time. On the host backend it only names the tools this machine lacks (exit 69 while any is missing). It then prints each vendor's install and login command for Claude Code, Codex, Cursor and Muse Code; it never installs or logs in to them.
- `toolu sandbox status` reports the backend, its state, the settings with their defaults and this project.
- `toolu sandbox start [--restart]`, `stop` and `delete --yes` manage the VM; on the host backend they change nothing.
- The `sandbox` skill tells agents when to run the gate, tests and host smoke in the sandbox, and what stays with the user: `setup`, agent CLI logins and restarting a running VM.

Exit codes: 0 done; 1 a step failed (the message names its log) or the config is invalid; 64 a usage error, `--json` on `run` or `shell`, `--clone` outside a git checkout, or a project directory that holds your home directory (list it in `sandbox.mounts` to allow it); 69 Lima is missing or older than 2, there is no VM yet, a setup never finished, the running VM does not mount this project, a tool is missing, or the command could not start; 75 another sandbox command is changing the VM. `run` and `shell` otherwise exit with the command's own status.

`toolu doctor` warns on macOS when `limactl` is missing.

## Muse

Muse Code loads this plugin from `.muse-plugin/plugin.json`. Validate it with `muse plugins validate plugins/sandbox`.
