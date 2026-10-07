# Installing toolu

`toolu` is the single native binary that the Rust rebuild ([epic #402](https://github.com/Falconiere/toolu/issues/402)) puts behind every plugin. The user installs it once. Plugins carry no binary and download nothing: their hooks and Markdown call the installed `toolu`. Decision record: [#411](https://github.com/Falconiere/toolu/issues/411). The Bun runtime stays the contract until the last TypeScript hook is gone (see [runtime.md](runtime.md)).

```bash
curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash
brew install falconiere/tap/toolu
```

## Install, upgrade and remove

The installer needs `bash`, `curl`, `tar`, and `sha256sum` or `shasum` (macOS also uses `unzip`). It installs the newest stable release that has assets for your platform into `/usr/local/bin`. Before it writes anything, it checks that `SHA256SUMS` carries the toolu release key's minisign signature made for that tag, that the archive matches its line, and that the archive holds exactly `toolu` and `LICENSE`. The new binary replaces the old one with a single `mv`, so a failed install leaves the previous binary in place. It never runs `sudo`, and it refuses to replace or remove a `toolu` that is Homebrew's link into its `Cellar` (Intel Homebrew also uses `/usr/local/bin`); use `brew upgrade toolu` or `brew uninstall toolu` for that one. When `/usr/local/bin` is not writable, it exits and names `--install-dir`:

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

The `get.toolu.sh` Worker answers `/pkg/toolu/install` with a 302 to `install.sh` at a pinned commit of this repository, never at `main` ([CodaSignal/toolu.sh#79](https://github.com/CodaSignal/toolu.sh/pull/79); the URL returns 404 until that change is deployed). The formula in `Falconiere/homebrew-tap` is regenerated from each stable release's `SHA256SUMS` (see [releases/native.md](releases/native.md)); `brew test toolu` checks `toolu --version`.

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
| Rollback | `install.sh --version <tag>` (#457) and pinning the plugin version in each host. The transition fallback to Bun bundles is in #425. |
| Upgrade | `brew upgrade toolu`, or re-run the installer. No self-update command in this version. |
| Removal | `brew uninstall toolu`, or `install.sh --uninstall`. `toolu doctor` names the right command for how the binary was installed. |
| Bun | Stops being a prerequisite on Claude Code and Codex when the last TypeScript hook is gone (#440). |

## Version skew

The binary and the plugins are upgraded separately (`brew upgrade`, the installer, each host's marketplace), so their versions differ. Compatibility is an integer hook-interface version, `hookProtocol`, declared in each `plugin.json` and compiled into the binary. It is bumped only when the `toolu hook` contract or a documented verb breaks.

| Case | Behaviour |
|------|-----------|
| Same `hookProtocol` | Hooks run and enforce. |
| Same `hookProtocol`, any semver difference (major included; a newer or an older binary) | Hooks still enforce. SessionStart prints one advisory: the upgrade command when the binary is older, an update of the plugins when it is newer or the versions cannot be compared. `feat!` releases such as #406 and #440 do not change `hookProtocol`. |
| Different `hookProtocol` | Enforcing events fail closed with the upgrade command. |

`toolu doctor` warns on skew and fails only on a `hookProtocol` mismatch.

**Compatibility.** While `hookProtocol` is unchanged, `toolu hook <event>` and every documented verb keep working, so a newer binary serves older plugins (#442).

**Config compatibility.** An older binary accepts and ignores unknown keys inside namespaced sections (for example `epic`, #463). Unknown top-level keys are still rejected.

The binary reads the calling plugin's `version` and `hookProtocol` from `.claude-plugin/plugin.json` (or `.codex-plugin/plugin.json` when the first is absent) under the root the launcher passes. The advisory prints once, at SessionStart. Its upgrade command is `brew upgrade toolu` when the binary resolves into a Homebrew prefix or a `Cellar`, and the installer one-liner otherwise. A missing or malformed manifest, a `version` that is not a string, or a `hookProtocol` that is not an integer of 1 or more, counts as a mismatch. The cases are tested through the real launcher in `crates/cli/tests/helpers/skew.rs`, and in unit tests beside `crates/core/runtime/src/{skew,version,manifest}.rs` and `crates/cli/src/hook.rs`; those cover the cases that need a protocol 2 binary or a malformed manifest.

## Hook launcher

Every native `hooks.json` entry runs one generated `sh -c` command (#412), printed by `cargo xtask print-hook <plugin> <Event> <name>` and gated by `cargo xtask check-hooks`. It never runs `toolu` through `PATH` alone.

1. **`TOOLU_BIN`.** When set, it is the only candidate. If it is missing, not executable, or not a native `toolu`, the hook fails closed instead of searching further. An empty `TOOLU_BIN` counts as unset.
2. **The install directories:** `/opt/homebrew/bin`, `/usr/local/bin`, `/home/linuxbrew/.linuxbrew/bin` and `~/.local/bin`, in that order. A host may run hooks with a reduced `PATH`, and these come first so a wrapper on `PATH` cannot shadow the binary.
3. **`toolu` on `PATH`.**

A candidate counts only if `toolu --hook-protocol` prints an integer. The npm wrapper `@toolu/plugins` also installs a `toolu` bin (until #438); it fails this check and is skipped.

| Situation | Enforcing event (`PreToolUse`, `PermissionRequest`) | Context event |
|---|---|---|
| Binary found | runs `toolu [<plugin>] hook <name> --event <Event> --plugin-root <root>`; exit 0 and 2 pass through, and any other status (abort, SIGSEGV, SIGKILL, OOM) becomes exit 2 with `blocked: <plugin> plugin: toolu ended with status <n>, so the action is blocked.` | `exec`s the binary |
| No binary, Bun and the plugin's `hooks/dist/<name>.js` present (transition, #425) | runs the Bun bundle, with one stderr line: `<plugin> plugin: native toolu not found, running the Bun bundle for this transition release - see #425. …` | same |
| No binary and no fallback, or a bad `TOOLU_BIN` | exit 2, stderr `blocked: <plugin> plugin: toolu is not installed - checked TOOLU_BIN, … Install it with: curl -fsSL https://get.toolu.sh/pkg/toolu/install \| bash, or: brew install falconiere/tap/toolu. …` | exit 0, the same text as one `{"systemMessage": …}` |

The fallback is skipped when `TOOLU_BIN` is set, and #440 removes it. The command string carries no version, so it stays the same across releases and Codex does not ask for hook trust again. Every generated entry declares a `timeout` (60 seconds by default, `--timeout` up to 600). Until the hook main wrapper keeps its own deadline (#413), a hook that hangs past it is a non-blocking host error. `commandWindows` keeps the Bun chain and prints the install hint, because Windows has no binary yet.

On session start and resume, toolu's `session-start` hook prints `toolu runtime: native <version> at <path>`.

**Verifying.** `cargo xtask launcher-e2e --bin <dir>/toolu` runs the real launcher against a binary installed in one of the fixed directories, under `PATH=/usr/bin:/bin`. CI runs it at Homebrew's bin and at `/usr/local/bin` on Linux and macOS. The launcher tests (`cargo test -p toolu-cli --test launcher`, and `launcher-e2e`'s own tests) fail on purpose when a real `toolu` sits in `/opt/homebrew/bin`, `/usr/local/bin` or `/home/linuxbrew/.linuxbrew/bin`, because that binary would win. On such a machine, move it aside or run the tests in a container.

## Agent command shell

Skills, commands and agents write plain `toolu …`, without a fixed binary path. A SessionStart check uses `sh -c 'command -v toolu'` with the agent command shell's `PATH` and accepts the result only when `--hook-protocol` identifies the native binary. If that command resolves the native binary, the check is silent. If the binary is only at `TOOLU_BIN`, Homebrew's bin, `/usr/local/bin`, or `~/.local/bin`, SessionStart gives its absolute path to use for that session. If none is present, it gives one advisory with both install commands above. Independent plugin hooks share a session marker, so the advice appears once even when several plugins start.

The npm plugin installer still publishes a Node `toolu` wrapper until #438. It does not pass the native protocol check. If that wrapper appears first on `PATH`, SessionStart supplies the native absolute path when one is installed in a known directory. Invoke the native binary's `doctor` command with that absolute path to check whether plain `toolu` resolves from a non-login shell; it exits 1 if the binary is missing or shadowed by the wrapper. A plain `toolu doctor` in the shadowed shell would run the npm wrapper. When `toolu` is the native binary, `toolu doctor` also reports runtime, host, config, plugins, skew, registry and tools. Hooks continue to use their generated launcher, which has its own resolution and fail-closed contract above.
