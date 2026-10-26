# Installing toolu

`toolu` is the single native binary that the Rust rebuild ([epic #402](https://github.com/Falconiere/toolu/issues/402)) puts behind every plugin. The user installs it once. Plugins carry no binary and download nothing: their hooks and Markdown call the installed `toolu`. Decision record: [#411](https://github.com/Falconiere/toolu/issues/411). No other runtime is needed: Bun stopped being a prerequisite on Claude Code and Codex in #440 (see [runtime.md](runtime.md)).

```bash
curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash
brew install falconiere/tap/toolu
```

## Install, upgrade and remove

The installer needs `bash`, `curl`, `tar`, and `sha256sum` or `shasum` (macOS also uses `unzip`). It installs the newest stable release that has assets for your platform into `/usr/local/bin`. Before it writes anything, it checks that `SHA256SUMS` carries the toolu release key's minisign signature made for that tag, that the archive matches its line, and that the archive holds exactly `toolu` and `LICENSE`. Releases up to v8.4.0 are signed by a retired key whose secret was lost; the installer still accepts that key for those tags only, so `--version` installs v8.4.0 and earlier, while v8.4.1 and later must carry the current key. The new binary replaces the old one with a single `mv`, so a failed install leaves the previous binary in place. It never runs `sudo`, and it refuses to replace or remove a `toolu` that is Homebrew's link into its `Cellar` (Intel Homebrew also uses `/usr/local/bin`); use `brew upgrade toolu` or `brew uninstall toolu` for that one. When `/usr/local/bin` is not writable, it exits and names `--install-dir`:

```bash
curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash -s -- --install-dir ~/.local/bin
```

| Task | Homebrew | Installer |
|------|----------|-----------|
| Upgrade | `brew upgrade toolu` | re-run `curl -fsSL https://get.toolu.sh/pkg/toolu/install \| bash` |
| Uninstall | `brew uninstall toolu` | `curl -fsSL https://get.toolu.sh/pkg/toolu/install \| bash -s -- --uninstall` |
| Pin a version | — | `curl -fsSL https://get.toolu.sh/pkg/toolu/install \| bash -s -- --version v1.2.3` |
| See the plan | — | `--check` prints the platform, version, directory and download URL, and downloads nothing |

Pass the same `--install-dir` to `--uninstall` when you installed elsewhere. `--version` accepts a `vX.Y.Z` tag and installs that tag only.

**Rollback.** Install the previous release over the current one:

```bash
curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash -s -- --version v1.2.2
```

**From a fork.** `TOOLU_REPO=<owner>/<name>` installs a fork's releases. It goes on the `bash` side of the pipe, and the fork's `SHA256SUMS` must still be signed by the toolu release key:

```bash
curl -fsSL https://get.toolu.sh/pkg/toolu/install | TOOLU_REPO=example/toolu bash
```

**Rate limits.** Finding the newest release asks GitHub's API, which allows 60 unauthenticated requests per hour per address. Behind a shared address, set `TOOLU_GITHUB_TOKEN` (or `GITHUB_TOKEN`) for that call; the installer sends it only to the API, never on its command line. `--version` skips the API altogether.

Exit codes: 0 installed, checked or removed; 1 a network, signature, checksum, archive or permission failure; 2 a usage error, a bad tag or `TOOLU_REPO`, or an unsupported platform (only Darwin and Linux, on arm64 or amd64).

## Run the epic engine as a Linux user service

Run `toolu epic service install` from the shell that can already run `gh` and `herdr` and reach the intended Herdr session. It writes `~/.config/systemd/user/toolu-epic.service` and a copy under the toolu resource root. The unit captures that shell's `PATH`, `HERDR_ENV`, `HERDR_SOCKET_PATH`, `HERDR_BIN_PATH`, `HERDR_CONFIG_PATH`, `HERDR_SESSION`, and `TOOLU_EPIC_HERDR_SESSION` when set. It restarts the engine after a failure, with a five-second delay. Re-run the install command and reload the user manager after any of those paths or variables change.

For Jev routing, create `~/.config/toolu/epic.env` with a line `TYPESAFE_API_KEY=<your key>` and set its mode to 0600 (`chmod 600 ~/.config/toolu/epic.env`). The unit reads this optional file when it starts; the key is never copied into the generated unit. Put `GH_TOKEN` here too if `gh auth` is unavailable to the user service. Keep the file private and avoid committing it. Without a Jev key, the engine uses its configured routing fallback.

After installation or a change to either file, run `systemctl --user daemon-reload`, then `systemctl --user enable --now toolu-epic.service` (or `systemctl --user restart toolu-epic.service` when it is already enabled). Check `systemctl --user status toolu-epic.service` and `journalctl --user -u toolu-epic.service` for startup errors. To keep the user manager running after logout, run `loginctl enable-linger "$(id -un)"` with administrator privileges if required. The install command only writes the unit; it does not start or enable it.

The `get.toolu.sh` Worker answers `/pkg/toolu/install` with a 302 to `https://github.com/Falconiere/toolu/releases/latest/download/install.sh`, the `install.sh` asset of the latest stable release, never `main` ([CodaSignal/toolu.sh#101](https://github.com/CodaSignal/toolu.sh/pull/101); it replaced the commit pin of [#79](https://github.com/CodaSignal/toolu.sh/pull/79)). Publishing a stable release therefore changes the installer the one-liner runs; drafts and prereleases do not. The formula in `Falconiere/homebrew-tap` is regenerated from each stable release's `SHA256SUMS` (see [releases/native.md](releases/native.md)); `brew test toolu` checks `toolu --version`.

## Why this way

- Neither Claude Code nor Codex has a mechanism for per-platform binaries; a plugin is a directory tree.
- The machinery is proven in `Falconiere/toolu-ghrunner`: release tarballs with `SHA256SUMS`, an installer served through `get.toolu.sh`, and a formula in `Falconiere/homebrew-tap`.
- Rejected: downloading the binary at SessionStart (network on first run, a race with the first hook), a separate distribution marketplace repository (every install carries every platform), and committing binaries here (tens of MB per release in git history).

## Policy

| Topic | Rule |
|-------|------|
| Targets | `darwin-arm64`, `darwin-amd64`, `linux-amd64`, `linux-arm64`. Linux binaries are statically linked (`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`, #456), so one binary runs on any distribution. Windows is deferred: `commandWindows` stays generated and prints the install hint. |
| Location | Homebrew's prefix, or `/usr/local/bin` (or `--install-dir`) for the installer. The hook launcher also looks in `~/.local/bin` (#412). |
| Missing binary | Enforcing hooks fail closed and name both install commands. Context hooks print one `systemMessage`. |
| macOS | curl and brew do not set quarantine. Finalize runs `codesign --verify` on darwin binaries (ad-hoc signature after strip). Notarization is deferred. |
| Rollback | `install.sh --version <tag>` (#457) and pinning the plugin version in each host. |
| Upgrade | `brew upgrade toolu`, or re-run the installer. `toolu doctor` names the right command for how the binary was installed. No self-update command in this version. |
| Removal | `brew uninstall toolu`, or `install.sh --uninstall`. |
| Bun | Not used or needed on Claude Code, Codex or Muse since #440. |

## Version skew

The binary and the plugins are upgraded separately (`brew upgrade`, the installer, each host's marketplace), so their versions differ. Compatibility is an integer hook-interface version, `hookProtocol`, declared in each `plugin.json` and compiled into the binary. It is bumped only when the `toolu hook` contract or a documented verb breaks.

| Case | Behaviour |
|------|-----------|
| Same `hookProtocol` | Hooks run and enforce. |
| Same `hookProtocol`, any semver difference (major included; a newer or an older binary) | Hooks still enforce. SessionStart prints one advisory: the upgrade command when the binary is older, an update of the plugins when it is newer or the versions cannot be compared. `feat!` releases such as #406 and #440 do not change `hookProtocol`. |
| Different `hookProtocol` | Enforcing events fail closed with the upgrade command. |

`toolu doctor` warns on skew and fails only on a `hookProtocol` mismatch.

**Retired plugins.** A plugin installed from the toolu marketplace that the catalog built into the binary no longer lists, and that is not newer than the binary, is retired: context7, exa-search, jira and agent-browser left in the Rust rewrite, and their 7.11.0 manifests carry no `hookProtocol`. No marketplace update can fix them, so doctor's skew check leaves them out of the mismatch and advice lists. It warns `retired plugin <name> is no longer in the toolu marketplace` and gives the command that removes it on the inspected host: `toolu plugins remove <names> --host <host> --yes` on Claude Code, Codex and Cursor, and `muse plugins remove <name>` on Muse. Once it is removed, the check passes. A plugin the catalog does not list but that is newer than the binary was released after this build, and keeps the upgrade advice (#615). `codex plugin list` omits plugins its marketplace no longer offers, so doctor run for Codex does not see them; `toolu plugins remove <name> --host codex --yes` still removes a leftover `[plugins."<name>@toolu"]` entry.

**Compatibility.** While `hookProtocol` is unchanged, `toolu hook <name>` and every documented verb keep working, so a newer binary serves older plugins (#442).

**Config compatibility.** An older binary accepts and ignores unknown keys inside namespaced sections (for example `epic`, #463). Unknown top-level keys are still rejected.

The binary reads the calling plugin's `version` and `hookProtocol` from `.claude-plugin/plugin.json` (or `.codex-plugin/plugin.json` when the first is absent) under the root the launcher passes. The advisory prints once, at SessionStart. Its upgrade command is `brew upgrade toolu` when the binary resolves into a Homebrew prefix or a `Cellar`, and the installer one-liner otherwise. For a plugin that is not retired, a missing or malformed manifest, a `version` that is not a string, or a `hookProtocol` that is not an integer of 1 or more, counts as a mismatch. The cases are tested through the real launcher in `crates/cli/tests/helpers/skew.rs`, and in unit tests beside `crates/core/runtime/src/{skew,version,manifest}.rs` and `crates/cli/src/hook.rs`; those cover the cases that need a protocol 2 binary or a malformed manifest.

## Hook launcher

Every native `hooks.json` entry runs one generated `sh -c` command (#412), printed by `cargo xtask print-hook <plugin> <Event> <name>` and gated by `cargo xtask check-hooks`. It never runs `toolu` through `PATH` alone.

1. **`TOOLU_BIN`.** When set, it is the only candidate. If it is missing, not executable, or not a native `toolu`, the hook fails closed instead of searching further. An empty `TOOLU_BIN` counts as unset.
2. **The install directories:** `/opt/homebrew/bin`, `/usr/local/bin`, `/home/linuxbrew/.linuxbrew/bin` and `~/.local/bin`, in that order. A host may run hooks with a reduced `PATH`, and these come first so a wrapper on `PATH` cannot shadow the binary.
3. **`toolu` on `PATH`.**

A candidate counts only if `toolu --hook-protocol` prints an integer. The deprecated npm package `@toolu/plugins` installs `toolu-plugins`, which exits 69 and is not a native `toolu`. A leftover `toolu` from a release before #438 fails this check and is skipped.

| Situation | Enforcing event (`PreToolUse`, `PermissionRequest`) | Context event |
|---|---|---|
| Binary found | runs `toolu [<plugin>] hook <name> --event <Event> --plugin-root <root>`; exit 0 and 2 pass through, and any other status (abort, SIGSEGV, SIGKILL, OOM) becomes exit 2 with `blocked: <plugin> plugin: toolu [<plugin>] hook <name> on <Event> ended with status <n>, so the action is blocked.` | runs the same command; exit 0 and 2 pass through, and any other status becomes exit 0 with `{"systemMessage":"<plugin> plugin: toolu [<plugin>] hook <name> on <Event> ended with status <n>. See docs/install.md."}` |
| No binary, or a bad `TOOLU_BIN` | exit 2, stderr `blocked: <plugin> plugin: no usable native toolu - checked TOOLU_BIN, … Install it with: curl -fsSL https://get.toolu.sh/pkg/toolu/install \| bash, or: brew install falconiere/tap/toolu. …` | exit 0, the same text as one `{"systemMessage": …}` |

There is no fallback: #440 removed the Bun bundles the transition release (#425) ran without a binary. The command string carries no version, so it stays the same across releases and Codex does not ask for hook trust again. Every generated entry declares a `timeout` (60 seconds by default, `--timeout` up to 600). Until the hook main wrapper keeps its own deadline (#413), a hook that hangs past it is a non-blocking host error. `commandWindows` reports the missing binary the same way, because Windows has no binary yet.

On session start and resume, toolu's native `session-start` hook prints `Toolu is on!` and then `toolu runtime: native <version> at <path>`. Every entry of every plugin's `hooks.json` uses the generated native launcher.

**Verifying.** `cargo xtask launcher-e2e --bin <dir>/toolu` runs the real launcher against a binary installed in one of the fixed directories, under `PATH=/usr/bin:/bin`. CI runs it at Homebrew's bin and at `/usr/local/bin` on Linux and macOS. The launcher tests (`cargo test -p toolu-cli --test launcher`, and `launcher-e2e`'s own tests) fail on purpose when a real `toolu` sits in `/opt/homebrew/bin`, `/usr/local/bin` or `/home/linuxbrew/.linuxbrew/bin`, because that binary would win. On such a machine, move it aside or run the tests in a container.

### Find a failing hook on Codex

Codex 0.164.0 shows a failed hook as `Hook failed` and one line, without the event, plugin or command ([openai/codex#27052](https://github.com/openai/codex/issues/27052)). The line is `hook exited with code <n>`, `hook exited without a status code` when a signal ended the process, or `hook timed out after <n>s`. Codex always prints the number, so a blank code is a display or copy artifact. On `SessionStart`, `UserPromptSubmit` and `PostToolUse`, Codex discards stderr unless the exit is 2. That is why the launcher reports a toolu context hook's other statuses as a `systemMessage`, which Codex shows as a hook warning that names the hook and its event (#642). A `Hook failed` line with no name therefore comes from another plugin, from a timeout, or from a signal that killed the launcher's own shell.

Codex saves no hook run in the session rollout or its log database. Its app server reports each run as a `hook/completed` notification, and the notification names the plugin. To capture one, start `codex app-server` in the session's working directory with the same `CODEX_HOME`, so the same plugins and trusted hooks load. Send these JSON-RPC lines on its stdin, one per line, and wait for each response before sending the next. `turn/start` uses the `result.thread.id` that `thread/start` returns, together with the prompt that triggered the failure. `SessionStart` hooks run on the first turn.

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"probe","version":"0"}}}
{"jsonrpc":"2.0","method":"initialized"}
{"jsonrpc":"2.0","id":2,"method":"thread/start","params":{"cwd":"<session directory>"}}
{"jsonrpc":"2.0","id":3,"method":"turn/start","params":{"threadId":"<thread id>","input":[{"type":"text","text":"<prompt>"}]}}
```

Pass its stdout through `jq -c 'select(.method == "hook/completed") | .params.run | {eventName, sourcePath, displayOrder, status, entries}'`. `sourcePath` is the `hooks.json` that registered the hook, and a plugin's file lies in that plugin's installed directory. `displayOrder` is the hook's place in Codex's run order, and `entries[].text` is the line the TUI showed. `codex exec` prints only `hook: <Event>` lines on stderr: they name the event, not the plugin.

### On Muse

Muse Code runs a hook as an argv from `.muse-plugin/plugin.json`, with no shell string and no matcher, and a hook whose program is missing does not block the tool. So toolu never puts bare `toolu` in a Muse entry. Each entry runs its own resolver file, which holds the launcher above, with `--plugin-root "${MUSE_PLUGIN_ROOT}"`:

```json
{ "id": "pre-tools", "event": "PreToolUse", "command": ["/bin/sh", "hooks/muse/pre-tools"], "timeoutMs": 60000 }
```

- `cargo xtask print-muse-hook <plugin> <Event> <name> [--timeout-ms N]` prints the entry, and `--resolver` prints the file for `plugins/<plugin>/hooks/muse/<name>`. `cargo xtask check-hooks` fails on an entry or a resolver that differs from these, a missing or out-of-range `timeoutMs` (default 60000, at most 600000), an event toolu does not handle on Muse, a duplicate id, a `hookProtocol` field, and a `hooks/muse/` file no entry runs.
- There is one resolver per hook, because Muse reads any relative argv word containing `/` as a package file and refuses two hooks sharing one. The files have no extension and start with a "generated" comment.
- Resolution is the same as above, except that Muse passes hooks only `HOME`, `PATH`, its own variables and a few basics. `TOOLU_BIN` never arrives, so on Muse the install directories come first, then `PATH`.
- The fail-closed table above holds unchanged. With no binary, `PreToolUse` and `PermissionRequest` exit 2 with `blocked: …` on stderr, which Muse blocks. Context events print the install `systemMessage` with exit 0.
- Muse installs the whole plugin directory, so the skew check reads the packaged `.claude-plugin/plugin.json`. A different `hookProtocol` blocks or advises exactly as on Claude Code. Muse manifests carry no `hookProtocol`: Muse warns on unknown top-level fields and rejects unknown hook-entry fields.

### Install, approve and verify on Muse

Muse keeps a plugin's hooks and agent definitions inert until they are approved. With the `toolu` binary installed and `muse` on `PATH`:

```bash
toolu plugins install --host muse --approve   # register the release checkout, install, approve
muse plugins list                             # each plugin installed from the `toolu` marketplace
muse plugins inspect toolu                    # trust state: approved, not "awaiting approval"
```

- Without `--approve`, `install` and `update` end with `awaiting approval: muse plugins approve <name>`; run that command, or re-run with `--approve`. Muse asks again when a hook definition changes.
- `toolu plugins install` shallow-clones the tag matching the binary and registers it as the Muse marketplace `toolu` ([installer guide](cli/installer.md#muse)). `get.toolu.sh` and Homebrew install only the binary and are unaffected.
- Today only the `toolu` plugin carries a `.muse-plugin/plugin.json`, and its `hooks` list is still empty; the other plugins and the hooks follow in #547. Until then a release without a plugin's manifest gives that plugin a failed step (`not available in marketplace`).

After installing a toolu plugin, run `toolu --host muse doctor`. The doctor reads Muse's installed-plugin list and inspects each toolu plugin's runtime capabilities. An installed plugin can be active in the list while its hooks still need approval; the doctor warns and gives the corresponding `muse plugins approve` command. It also warns when a plugin is disabled and gives `muse plugins enable`, or when Muse's plugin inventory cannot be read. Run doctor again after approval to verify that the hooks are effective.

Muse user settings live under `TOOLU_USER_CONFIG_DIR` when set; otherwise the config root is `TOOLU_CONFIG_DIR`, `MUSE_CONFIG_DIR`, `XDG_CONFIG_HOME/muse`, or `~/.config/muse`, in that order. Project settings live at `<project>/.agents/toolu.config.json`. `toolu --host muse config set <key> <value>` writes the user file, and `--project` writes the project file. `toolu --host muse setup agents preview` still targets `CODEX_HOME/agents`.

## Agent command shell

Skills, commands and agents write plain `toolu …`, without a fixed binary path. A SessionStart check uses `sh -c 'command -v toolu'` with the agent command shell's `PATH` and accepts the result only when `--hook-protocol` identifies the native binary. If that command resolves the native binary, the check is silent. If the binary is only at `TOOLU_BIN`, Homebrew's bin, `/usr/local/bin`, or `~/.local/bin`, SessionStart gives its absolute path to use for that session. If none is present, it gives one advisory with both install commands above. Independent plugin hooks share a session marker, so the advice appears once even when several plugins start.

The npm package `@toolu/plugins` is deprecated. Its command is `toolu-plugins`, and it exits 69 instead of installing plugins. Releases before #438 named that command `toolu`; `npm uninstall -g @toolu/plugins` removes either leftover. If a leftover `toolu` appears first on `PATH`, SessionStart supplies the native absolute path when one is installed in a known directory. Invoke the native binary's `doctor` command with that absolute path to check whether plain `toolu` resolves from a non-login shell; it exits 1 if the binary is missing or shadowed. A plain `toolu doctor` in the shadowed shell would run the leftover command. When `toolu` is the native binary, `toolu doctor` also reports runtime, host, config, plugins, skew, registry and tools. Hooks continue to use their generated launcher, which has its own resolution and fail-closed contract above.
