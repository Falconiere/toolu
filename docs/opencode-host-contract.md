# OpenCode host contract

**Issue:** [#335](https://github.com/Falconiere/toolu/issues/335) (OP-01, epic [#334](https://github.com/Falconiere/toolu/issues/334))
**Status:** Documented contract pinned and probed on a real host.

The shipped adapter (`tools/toolu-opencode/src/plugin/toolu.ts`) still targets the superseded `@opencode/plugin@2.0.12` API. It does not conform to this contract. OP-02 ([#336](https://github.com/Falconiere/toolu/issues/336)) replaces it.

This contract is the plugin API documented at <https://opencode.ai/docs/plugins/>: a module exports a function that receives the plugin input and returns `Hooks`. Every statement below is backed by a live probe of the pinned host (see [Probe results](#probe-results)) or by the pinned SDK declarations. `bun run check:opencode-host` keeps the generated sections in sync with that evidence.

## Pin

| Component | Pin | Source |
|---|---|---|
| OpenCode CLI | `opencode-ai@1.18.34` | npm `opencode-ai`; its platform binary is `opencode-<os>-<arch>`. The probe installs it with `bun add --exact` into `$XDG_CACHE_HOME/toolu/opencode-host/1.18.34/cli`, never onto `PATH` |
| Plugin SDK | `@opencode-ai/plugin@1.18.34` | npm. The host writes the same version into each config directory's `package.json` and installs it there; `@toolu/opencode` pins it as a devDependency |
| Documentation | <https://opencode.ai/docs/plugins/> | Plugin function returning `Hooks` |
| Platform probed | linux-x64 | macOS acceptance belongs to OP-28 ([#362](https://github.com/Falconiere/toolu/issues/362)); Windows is not probed |
| Probe runner | Bun 1.4.x | The host binary embeds its own runtime |

The machine-readable pin is `tools/toolu-opencode/contract/pin.json`. `bun run check:opencode-host` fails when the adapter devDependency, the installed SDK or the recorded probe results disagree with it.

## Plugin shape

```ts
import type { Plugin } from "@opencode-ai/plugin";

export const TooluPlugin: Plugin = async ({ client, project, directory, worktree, serverUrl, $ }, options) => {
  return {
    /* Hooks */
  };
};
```

- The function receives `$`, `client`, `directory`, `experimental_workspace`, `project`, `serverUrl` and `worktree`.
- `options` is the second element of a config tuple `["spec", { … }]`. It is `undefined` for files discovered in `.opencode/plugins/`.
- A module may instead export one default `PluginModule` (`{ id, server }`). The host then calls only `server` and ignores named exports.
- Without a default `PluginModule`, the host calls every exported function as a plugin, with the plugin input. A helper function that returns no hooks breaks every prompt in the session. A non-function named export makes the whole module fail to load.

## Loader behavior

- **Discovery.** The host loads `.opencode/plugins/*.{ts,js}`, `~/.config/opencode/plugins/`, and `opencode.json` `plugin` entries. An entry is an npm spec, a path relative to the declaring config, a `file://` URL, or a `[spec, options]` tuple. Load order follows the official docs: global config, project config, global plugin directory, then project plugin directory.
- **SDK provisioning.** At startup the host writes `package.json` (`@opencode-ai/plugin` at its own version) and a `.gitignore` into each config directory, such as `.opencode/` and `~/.config/opencode/`, then installs it. The first start in a fresh profile needs registry access.
- **Fail-open init.** If a plugin's initialization throws, the host logs `failed to load plugin`, keeps the session running, and runs tools without that plugin. A gate plugin must therefore never throw from init: setup failures have to become deny-all hooks.
- **Project root.** The host takes its project directory from `PWD`, not from the process working directory.
- **Non-interactive runs.** `opencode run` reads a non-TTY stdin as extra input and blocks on it, so automation must close stdin.
- **Opting out.** `--pure` and `OPENCODE_PURE=1` skip external plugins entirely. A user can always run without toolu; that is the user's choice and outside enforcement.
- **Skill name validation.** The host loads skill names that break the documented rule (lowercase, single hyphens, matching the folder) without complaint, so generated names must validate themselves.

## Host surface

Hook callbacks run inline during the host's own flow and can mutate their `output`. `tool.execute.before` can also refuse by throwing. Event notifications arrive through the `event` hook after the fact and cannot block anything. The table lists every member of the pinned SDK's `Hooks` interface.

| Hook | Kind | Blocks? | Reaches the model | Probe / note |
|---|---|---|---|---|
| `tool.execute.before` | hook callback (mutate `args`, throw to refuse) | **Yes**: a throw stops the call before any side effect, and the message becomes the tool error | The throw message | `deny.bash`, `deny.write`, `deny.apply-patch`, `deny.mcp`, `deny.task-child`. Adding output fields gives no advisory channel (`pre.advisory`) |
| `tool.execute.after` | hook callback (mutate `title`, `output`, `metadata`) | No; it runs after the tool | Appended output | `post.feedback`, `post.bash-exit`. It does not run when a tool throws (`post.tool-error`). MCP output is `{ content }` |
| `permission.ask` | hook callback (declared) | — | — | Never invoked on the pin (`permission.ask-hook`) |
| `chat.message` | hook callback (mutate `message`, `parts`) | No | Added parts | `context.prompt` |
| `experimental.chat.system.transform` | hook callback (experimental) | No | System prompt lines | `context.system` |
| `experimental.session.compacting` | hook callback (experimental) | No | Compaction prompt context | `context.compaction` |
| `shell.env` | hook callback (mutate `env`) | No | Indirectly, through bash | `env.shell` |
| `command.execute.before` | hook callback (mutate `parts`) | Not probed | Command parts | `command.hook` |
| `config` | callback with the merged config at init | No | Commands, agents, `skills.paths`, `instructions` | `surface.config-hook`. `skills` is accepted by the host but absent from the SDK `Config` type |
| `event` | event notifications (`session.*`, `message.*`, `permission.asked`/`replied`, `command.executed`, `session.compacted`, `tui.toast.show`, …) | No | No | `events.bus`, `ui.toast` |
| `tool` | object of custom tool definitions | — | Adds tools | Not used by toolu yet |
| `auth`, `provider` | provider and auth registration | — | — | Not used by toolu |
| `chat.params`, `chat.headers` | hook callbacks (LLM parameters and headers) | No | — | Not used by toolu |
| `tool.definition` | hook callback (tool description and parameters) | No | Tool descriptions | Not used by toolu |
| `experimental.chat.messages.transform`, `experimental.provider.small_model`, `experimental.compaction.autocontinue`, `experimental.text.complete` | experimental hook callbacks | No | — | Not used by toolu |
| `dispose` | cleanup callback | — | — | Release resources on shutdown |

## Permission composition

- The user's `opencode.json` `permission` rules stay authoritative.
  - `ask` prompts in the TUI. `opencode run` rejects the ask unless `--auto` is passed.
  - `deny` removes the tool from the model's tool list. A plugin cannot override it (`permission.config-deny`).
- `tool.execute.before` runs before the native prompt (`permission.order`). A toolu deny therefore stops the call before the user is ever asked. A toolu allow adds no permission, so it can never bypass the user's rules.
- A plugin cannot open a native prompt, because the declared `permission.ask` hook is never invoked. Gate `ask` decisions degrade under the `@toolu/core/host` class rules: security guardrails deny, and judgement gates advise through `tool.execute.after`. OP-05 ([#339](https://github.com/Falconiere/toolu/issues/339)) owns this.

## Probe results

`bun run probe:opencode-host` produced this table against the pinned host in isolated profiles.

- **Isolation.** Each probe gets a temporary `HOME` and `XDG_*` config, data and state, plus a fresh git project.
- **Model.** A loopback OpenAI-compatible server returns scripted tool calls, and `enabled_providers` admits only that provider.
- **Integration.** Plugin loading, permissions, tools, MCP (a stdio fixture server) and child sessions are the host's own code paths.
- **Evidence.** Side effects are checked on disk. What reached the model is checked in the requests the scripted server recorded.

<!-- opencode-host-probes:start -->
<!-- opencode-host-probes:end -->

## Capability matrix

The matrix covers all 16 catalog plugins across nine axes. Each needed axis has an owner work package. `check:opencode-host` derives the minimum axes from each plugin's own manifests (`hooks/hooks.json` events and matchers, `hooks/src/register.ts` registry modules), checks surface counts against `skills/`, `commands/` and `agents/`, and requires every status to agree with the probe verdicts it cites. Source: `tools/toolu-opencode/contract/capability-matrix.json`.

<!-- opencode-host-matrix:start -->
<!-- opencode-host-matrix:end -->

## Release blockers and limitations

When the pinned host does not support a required capability, the capability either blocks release or has an alternative backed by supported probes. An enforcement capability (one that decides whether a tool call runs) needs `alternativeEvidence` whose probes are all supported; prose alone never satisfies the check. Every unsupported probe verdict is assigned to an owner.

<!-- opencode-host-limitations:start -->
<!-- opencode-host-limitations:end -->

## Experimental API strategy

The CLI and SDK are pinned exactly and move together, because the host provisions the SDK at its own version.

- **Allowed use.** `experimental.*` hooks may deliver context: session protocol, compaction state. They are never used for enforcement, which relies only on `tool.execute.before` and `tool.execute.after`.
- **Pin bump.**
  1. Change `pin.json` and the adapter devDependency.
  2. Run `bun install` and `bun run probe:opencode-host`. Any verdict or observation drift fails.
  3. Review each drift and refresh the evidence with `--write`.
  4. Run `bun run check:opencode-host --write-doc`.
  5. Re-check every matrix cell whose evidence changed.

  The experimental hooks the matrix relies on are listed in the generated limitations above.

## Reproduce

```bash
bun run probe:opencode-host            # live: install the pinned CLI into the cache, run every probe, compare with the committed results
bun run probe:opencode-host --write    # refresh tools/toolu-opencode/contract/probe-results.json
bun run check:opencode-host            # hermetic: pins, evidence, matrix, manifests, SDK declarations and this doc agree
bun run check:opencode-host --write-doc
```

- **Overrides.** `TOOLU_OPENCODE_HOST_BIN` uses an existing binary, which must report the pinned version. `TOOLU_OPENCODE_HOST_CACHE` moves the cache.
- **What it never touches.** The probe never reads or writes the user's `~/.config/opencode`, never uses an `opencode` on `PATH`, and never calls a real model provider.
- **CI.** Live probing is opt-in until OP-28 ([#362](https://github.com/Falconiere/toolu/issues/362)) makes it mandatory. `check:opencode-host` runs in `bun run test`.
