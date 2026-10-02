# OpenCode documented host contract (OP-01, #335) — Brainstorm

**Date:** 2026-10-02   **Mode:** Delivery, Full path   **Issue:** [#335](https://github.com/Falconiere/toolu/issues/335) (epic [#334](https://github.com/Falconiere/toolu/issues/334))

## Capsule

- **Outcome:** a pinned OpenCode host and plugin SDK that match <https://opencode.ai/docs/plugins/>, a repeatable live probe harness, committed probe evidence, and a checked 16-plugin capability matrix. Each required behavior has an owner work package and an evidence-backed status.
- **Material defaults/non-goal:** pin `opencode-ai@1.18.34` with `@opencode-ai/plugin@1.18.34`. Drive the real host with a loopback scripted OpenAI-compatible provider. Keep the live run in a script and check its committed evidence hermetically in `bun run test`. The adapter migration (OP-02 onward) and mandatory live CI (OP-28) are out of scope.
- **Repository evidence:** memory `ce014dd7` (the V2 adapter targets a different API). `tooling/src/opencode-capability-probe.ts` records a hardcoded V2 matrix and only checks `--version`. `tools/toolu-opencode/src/plugin/toolu.ts` uses `Plugin.define` and `permission.hook("evaluate")`.
- **Risk:** experimental hooks (`experimental.chat.system.transform`, `experimental.session.compacting`) can change between host releases. A first live run needs network access for the host and its config-dir SDK install. Headless probes cannot prove that TUI rendering works.
- **Handoff:** spec.

## Probe evidence (scratch run, pinned host)

The probes ran against `opencode-ai@1.18.34` (linux-x64, Bun 1.4.2) with an isolated `HOME`/`XDG_*` profile and a git project. A loopback server returned scripted tool calls.

| Behavior | Observation |
|---|---|
| Local-file load | A typed `Plugin` in `.opencode/plugins/` received `{client, project, directory, worktree, serverUrl, $, experimental_workspace}`, with `options` undefined |
| Config load | `plugin: [["file:///abs/probe.ts", {…}]]` loaded the plugin and passed the options tuple as the second argument |
| Init throw | The host logged `failed to load plugin`, continued, and ran the tool unguarded (fail-open) |
| Exported helper function | Invoked as a plugin with `PluginInput`. Its non-hooks return broke every prompt (`UnknownError`). A non-function export fails the whole module (`Plugin export is not a function`) |
| Default `PluginModule` | Only `server` was invoked; named exports were ignored |
| `tool.execute.before` throw | Blocked bash, write, `apply_patch` (multi-file, atomic), MCP `probe_touch`, and a `task` child-session bash with no side effects. The error text reached the model, and the after hook did not run |
| `permission.ask` hook | Never invoked, even with config `bash: ask`. `opencode run` auto-rejects asks unless `--auto` is set; config `deny` removes the tool |
| Order | `tool.execute.before` runs before `permission.asked` |
| `tool.execute.after` | Appending to `output.output` reached the next model request. It fires for a bash exit 3 (`metadata.exit`), but not for a thrown tool error (`read` of a missing file) |
| Context | `experimental.chat.system.transform`, `chat.message` parts and `experimental.session.compacting` (via `POST /session/:id/summarize`) all reached the provider request |
| `shell.env`, `command.execute.before`, `config` hook | The env var reached bash. The command hook received the arguments. The config hook injected a command, an agent, `skills.paths` and `instructions` |
| Surfaces | Project skills, agents and commands were discovered. The host loaded the invalid skill names `toolu--double` and `Bad_Name` without complaint |
| UI | `client.tui.showToast` published `tui.toast.show` on the bus. TUI rendering was not observable headless |
| Loader install | The host wrote `.opencode/package.json` with `@opencode-ai/plugin: 1.18.34`, equal to the host version, and installed it (with `package-lock.json`) |
| Stdin | `opencode run` blocks on non-TTY stdin; probes must close stdin |

## Alternatives (Jev, 2026-10-02)

| Decision | Chosen | Rejected | Jev |
|---|---|---|---|
| Pin | `opencode-ai@1.18.34` + `@opencode-ai/plugin@1.18.34` | locally installed `opencode v2.0.21` / `@opencode/plugin@2.0.21`; keep `v2.0.12` | v1docs 1.00 |
| Probe driver | Loopback scripted OpenAI-compatible provider driving the real host | real LLM provider (credentials, nondeterministic); direct adapter calls (no host loading) | scripted 0.98 |
| CI placement | Live probe script that refreshes committed evidence, plus a hermetic consistency check in `bun run test` | live host in every CI run now (that is OP-28's scope, and CI is hermetic) | script-plus-check 1.00 |

## Defaults and open risks

- The typed probe plugins live in the adapter workspace, outside the published `files`, so host SDK types stay in the adapter. Lint bans now cover `@opencode-ai/*` in core, tooling and conformance.
- The host is fail-open on plugin init failure and invokes every exported function. OP-02 must export only plugin functions or one default `PluginModule`, and must convert init errors into deny-all hooks.
- The host has no plugin-initiated ask, so OP-05 must degrade `ask` using the existing `@toolu/core/host` class rules.
