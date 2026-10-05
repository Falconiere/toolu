# Installing toolu

`toolu` is the single native binary that the Rust rebuild ([epic #402](https://github.com/Falconiere/toolu/issues/402)) puts behind every plugin. The user installs it once. Plugins carry no binary and download nothing: their hooks and Markdown call the installed `toolu`. Decision record: [#411](https://github.com/Falconiere/toolu/issues/411). The Bun runtime stays the contract until the last TypeScript hook is gone (see [runtime.md](runtime.md)).

```bash
curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash
brew install falconiere/tap/toolu
```

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
| Same `hookProtocol`, any semver difference (major included; a newer or an older binary) | Hooks still enforce. Context events print one advisory with the upgrade command. `feat!` releases such as #406 and #440 do not change `hookProtocol`. |
| Different `hookProtocol` | Enforcing events fail closed with the upgrade command. |

`toolu doctor` warns on skew and fails only on a `hookProtocol` mismatch.

**Compatibility.** While `hookProtocol` is unchanged, `toolu hook <event>` and every documented verb keep working, so a newer binary serves older plugins (#442).

**Config compatibility.** An older binary accepts and ignores unknown keys inside namespaced sections (for example `epic`, #463). Unknown top-level keys are still rejected.

The five skew cases are tested in #412.
