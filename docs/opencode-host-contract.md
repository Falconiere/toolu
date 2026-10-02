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
Recorded 2026-10-02 on `opencode-ai@1.18.34` (linux-x64, Bun 1.4.2); the host provisioned `@opencode-ai/plugin@1.18.34`. Install: npm:opencode-ai@1.18.34 (bun add --exact).

| Probe | Axis | Kind | Mechanism | Claim | Verdict |
|---|---|---|---|---|---|
| `load.local-file` | load | loader | .opencode/plugins/*.ts | A typed plugin file in .opencode/plugins/ is loaded and called with the documented PluginInput | ✅ supported |
| `load.config-file` | load | config | opencode.json plugin: [["file://…", options]] | A plugin listed in opencode.json is loaded with its options and its hooks are active | ✅ supported |
| `load.module-default` | load | loader | export default { id, server } (PluginModule) | A default PluginModule export is loaded through its server function only | ✅ supported |
| `load.init-throw` | load | loader | plugin function throws during init | The host refuses to run tools when a plugin fails to initialize (fail closed) | ❌ unsupported |
| `load.helper-export` | load | loader | module exports a plugin and a helper function | The loader ignores exported functions that are not plugins | ❌ unsupported |
| `deny.bash` | tools | hook | tool.execute.before (throw) | A tool.execute.before throw blocks a bash call before it runs and the model sees the reason | ✅ supported |
| `deny.write` | tools | hook | tool.execute.before (throw) | A tool.execute.before throw blocks a write before the file changes | ✅ supported |
| `deny.apply-patch` | tools | hook | tool.execute.before (throw) | A tool.execute.before throw blocks a whole multi-file apply_patch (gpt-* models) before any file changes | ✅ supported |
| `deny.mcp` | mcp | hook | tool.execute.before on <server>_<tool> | A tool.execute.before throw blocks an MCP tool call before the server receives it | ✅ supported |
| `deny.task-child` | task | hook | tool.execute.before on task + child session | tool.execute.before sees the task tool and child-session tools, and its throw blocks them | ✅ supported |
| `pre.advisory` | tools | hook | tool.execute.before output fields | tool.execute.before can add non-blocking advisory context that reaches the model | ❌ unsupported |
| `permission.ask-hook` | permission | hook | permission.ask | The plugin permission.ask hook is consulted when the host asks for permission | ❌ unsupported |
| `permission.config-deny` | permission | config | opencode.json permission deny | A user's config deny cannot be overridden by a plugin | ✅ supported |
| `permission.order` | permission | hook | tool.execute.before vs permission.asked | tool.execute.before runs before the native permission prompt | ✅ supported |
| `post.feedback` | postTool | hook | tool.execute.after (output append) | tool.execute.after can append feedback that reaches the model | ✅ supported |
| `post.bash-exit` | postTool | hook | tool.execute.after metadata.exit | tool.execute.after runs for a bash command that exits non-zero and reports its exit code | ✅ supported |
| `post.tool-error` | postTool | hook | tool.execute.after on a thrown tool error | tool.execute.after runs when a tool call fails | ❌ unsupported |
| `context.system` | startup | hook | experimental.chat.system.transform | experimental.chat.system.transform adds system context that reaches the model | ✅ supported |
| `context.prompt` | prompt | hook | chat.message (output.parts) | chat.message can add a text part that reaches the model with the user's prompt | ✅ supported |
| `context.compaction` | compaction | hook | experimental.session.compacting | experimental.session.compacting adds context that reaches the compaction request | ✅ supported |
| `env.shell` | env | hook | shell.env | shell.env injects environment variables into bash tool calls | ✅ supported |
| `command.hook` | surfaces | hook | command.execute.before + command.executed | command.execute.before runs for project commands with their arguments | ✅ supported |
| `surface.files` | surfaces | surface | .opencode/{skills,agents,commands}/ | Project skills, agents and commands under .opencode/ are discovered | ✅ supported |
| `surface.names` | surfaces | surface | skill name validation | The host rejects skill names that violate the documented naming rule | ❌ unsupported |
| `surface.config-hook` | surfaces | config | config hook | A plugin config hook can inject commands, agents, skill paths and instructions | ✅ supported |
| `ui.toast` | ui | event | client.tui.showToast → tui.toast.show | A server plugin can publish a TUI toast through client.tui.showToast | ✅ supported |
| `events.bus` | startup | event | event hook | The event hook receives bus notifications such as session.created, message.updated and session.idle | ✅ supported |
<!-- opencode-host-probes:end -->

## Capability matrix

The matrix covers all 16 catalog plugins across nine axes. Each needed axis has an owner work package. `check:opencode-host` derives the minimum axes from each plugin's own manifests (`hooks/hooks.json` events and matchers, `hooks/src/register.ts` registry modules), checks surface counts against `skills/`, `commands/` and `agents/`, and requires every status to agree with the probe verdicts it cites. Source: `tools/toolu-opencode/contract/capability-matrix.json`.

<!-- opencode-host-matrix:start -->
✅ supported · 🟡 partial · ❌ unsupported · — not needed · 🔒 enforcement (decides whether a tool call runs)

| Plugin | tools | permission | startup | prompt | compaction | postTool | mcp | task | ui | Surfaces (skills/commands/agents) |
|---|---|---|---|---|---|---|---|---|---|---|
| agent-browser | ✅ | — | ✅ | — | — | — | — | — | — | 1/0/0 |
| ast-grep | ❌ | — | ✅ | — | — | ✅ | — | — | — | 1/0/0 |
| brainstorm | — | — | — | — | — | — | — | — | — | 1/0/0 |
| context7 | ✅ | — | ✅ | — | — | — | — | — | — | 1/0/0 |
| delivery-flow | ✅ | — | — | — | — | — | — | ✅ | — | 1/0/0 |
| epic-orchestrator | ✅ | — | ✅ | — | — | — | — | ✅ | — | 1/1/0 |
| exa-search | ✅ | — | ✅ | — | — | — | — | — | — | 1/0/0 |
| jev | ✅ | — | ✅ | ✅ | — | — | — | — | — | 1/0/0 |
| jira | ✅ | — | ✅ | — | — | — | — | — | — | 1/0/0 |
| pr-babysit | ✅ | — | ✅ | — | — | — | — | ✅ | — | 1/1/0 |
| python-quality | — | — | ✅ | — | — | ✅ | — | — | — | 0/0/0 |
| rust-quality | — | — | ✅ | — | — | ✅ | — | — | — | 0/0/0 |
| statusline | ✅ | — | ✅ | — | — | — | — | — | ✅ | 1/1/0 |
| toolu | ✅🔒 | 🟡🔒 | ✅ | ✅ | ✅ | 🟡 | ✅🔒 | ✅🔒 | — | 6/2/5 |
| toolu-review | ✅ | — | ✅ | — | — | — | — | — | — | 1/0/0 |
| ts-quality | — | — | ✅ | — | — | ✅ | — | — | — | 0/0/0 |

- **agent-browser** (owner OP-12 (#346))
  - **tools** — agent-browser CLI runs through bash from the published helper path. `bash tool + shell.env (helper path and environment)` (tool): supported; evidence `env.shell`. Owner: OP-12 (#346), OP-09 (#343).
  - **startup** — SessionStart publishes the agent-browser helper and bounded browser instructions. `plugin init + experimental.chat.system.transform` (hook): supported; evidence `load.local-file`, `context.system`. Owner: OP-12 (#346), OP-07 (#341), OP-08 (#342).
  - **surfaces** — 1 skills, 0 commands, 0 agents. Owner: OP-10 (#344), OP-11 (#345), OP-12 (#346).
- **ast-grep** (owner OP-13 (#347))
  - **tools** — search-nudge advises ast-grep before Grep or bash text search, without blocking. `tool.execute.before (no advisory channel)` (hook): unsupported; evidence `pre.advisory`. Alternative: Deliver the nudge with the tool result through tool.execute.after. Owner: OP-13 (#347), OP-05 (#339).
  - **startup** — SessionStart registers the search-nudge (pre) and byte-savings (post) registry modules. `plugin init (bootstrap registry)` (hook): supported; evidence `load.local-file`. Owner: OP-13 (#347), OP-08 (#342).
  - **postTool** — byte-savings reports search output savings after the tool runs. `tool.execute.after (append to output)` (hook): supported; evidence `post.feedback`. Owner: OP-13 (#347), OP-06 (#340).
  - **surfaces** — 1 skills, 0 commands, 0 agents. Owner: OP-10 (#344), OP-11 (#345), OP-13 (#347).
- **brainstorm** (owner OP-21 (#355))
  - **surfaces** — 1 skills, 0 commands, 0 agents. Owner: OP-10 (#344), OP-11 (#345), OP-21 (#355).
- **context7** (owner OP-14 (#348))
  - **tools** — context7 search helper runs through bash. `bash tool + shell.env (helper path and environment)` (tool): supported; evidence `env.shell`. Owner: OP-14 (#348), OP-09 (#343).
  - **startup** — SessionStart publishes the context7 search helper and the documentation-first instructions. `plugin init + experimental.chat.system.transform` (hook): supported; evidence `load.local-file`, `context.system`. Owner: OP-14 (#348), OP-07 (#341), OP-08 (#342).
  - **surfaces** — 1 skills, 0 commands, 0 agents. Owner: OP-10 (#344), OP-11 (#345), OP-14 (#348).
- **delivery-flow** (owner OP-21 (#355))
  - **tools** — plan-ledger and verdict CLIs run through bash with TOOLU_PLUGIN_ROOT. `bash tool + shell.env (helper path and environment)` (tool): supported; evidence `env.shell`. Owner: OP-21 (#355), OP-09 (#343).
  - **task** — Phases delegate bounded work to subagents. `task tool + child sessions` (tool): supported; evidence `deny.task-child`. Owner: OP-21 (#355).
  - **surfaces** — 1 skills, 0 commands, 0 agents. Owner: OP-10 (#344), OP-11 (#345), OP-21 (#355).
- **epic-orchestrator** (owner OP-22 (#356))
  - **tools** — Report, watch and tracker scripts run through bash. `bash tool + shell.env (helper path and environment)` (tool): supported; evidence `env.shell`. Owner: OP-22 (#356), OP-09 (#343).
  - **startup** — SessionStart reports missing dependency plugins. `plugin init + experimental.chat.system.transform` (hook): supported; evidence `load.local-file`, `context.system`. Owner: OP-22 (#356), OP-08 (#342).
  - **task** — Workers are routed to OpenCode agents per sub-issue. `agents (.opencode/agents) + task tool` (tool): supported; evidence `surface.files`, `deny.task-child`. Owner: OP-22 (#356).
  - **surfaces** — 1 skills, 1 commands, 0 agents. Owner: OP-10 (#344), OP-11 (#345), OP-22 (#356).
- **exa-search** (owner OP-15 (#349))
  - **tools** — exa-search helper runs through bash. `bash tool + shell.env (helper path and environment)` (tool): supported; evidence `env.shell`. Owner: OP-15 (#349), OP-09 (#343).
  - **startup** — SessionStart publishes the exa-search helper and the web-research instructions. `plugin init + experimental.chat.system.transform` (hook): supported; evidence `load.local-file`, `context.system`. Owner: OP-15 (#349), OP-07 (#341), OP-08 (#342).
  - **surfaces** — 1 skills, 0 commands, 0 agents. Owner: OP-10 (#344), OP-11 (#345), OP-15 (#349).
- **jev** (owner OP-16 (#350))
  - **tools** — jev.sh runs through bash with the resolved Bun. `bash tool + shell.env (helper path and environment)` (tool): supported; evidence `env.shell`. Owner: OP-16 (#350), OP-09 (#343).
  - **startup** — SessionStart publishes jev.sh and the mandatory-judgment instructions. `plugin init + experimental.chat.system.transform` (hook): supported; evidence `load.local-file`, `context.system`. Owner: OP-16 (#350), OP-07 (#341), OP-08 (#342).
  - **prompt** — UserPromptSubmit reminds the agent to call Jev before semantic decisions. `chat.message (output.parts)` (hook): supported; evidence `context.prompt`. Owner: OP-16 (#350), OP-07 (#341).
  - **surfaces** — 1 skills, 0 commands, 0 agents. Owner: OP-10 (#344), OP-11 (#345), OP-16 (#350).
- **jira** (owner OP-17 (#351))
  - **tools** — jira helper runs through bash. `bash tool + shell.env (helper path and environment)` (tool): supported; evidence `env.shell`. Owner: OP-17 (#351), OP-09 (#343).
  - **startup** — SessionStart publishes the jira helper and issue-workflow instructions. `plugin init + experimental.chat.system.transform` (hook): supported; evidence `load.local-file`, `context.system`. Owner: OP-17 (#351), OP-07 (#341), OP-08 (#342).
  - **surfaces** — 1 skills, 0 commands, 0 agents. Owner: OP-10 (#344), OP-11 (#345), OP-17 (#351).
- **pr-babysit** (owner OP-23 (#357))
  - **tools** — Tick, collect, reply, resolve and record bundles run through bash. `bash tool + shell.env (helper path and environment)` (tool): supported; evidence `env.shell`. Owner: OP-23 (#357), OP-09 (#343).
  - **startup** — SessionStart checks that toolu is installed. `plugin init + experimental.chat.system.transform` (hook): supported; evidence `load.local-file`, `context.system`. Owner: OP-23 (#357), OP-08 (#342).
  - **task** — Fixes are dispatched to fixer agents. `task tool + child sessions` (tool): supported; evidence `deny.task-child`. Owner: OP-23 (#357).
  - **surfaces** — 1 skills, 1 commands, 0 agents. Owner: OP-10 (#344), OP-11 (#345), OP-23 (#357).
  - **note** — Tick scheduling: the documented plugin API has no cron or loop primitive, so babysit ticks need a host-native scheduler or an external loop. Owner: OP-23 (#357).
- **python-quality** (owner OP-19 (#353))
  - **startup** — SessionStart registers the Python post-tools.d module. `plugin init (bootstrap registry)` (hook): supported; evidence `load.local-file`. Owner: OP-19 (#353), OP-08 (#342).
  - **postTool** — Post-edit Python quality checks reach the model and gate later commit/push. `tool.execute.after (append to output)` (hook): supported; evidence `post.feedback`, `post.bash-exit`. Owner: OP-19 (#353), OP-06 (#340).
  - **surfaces** — 0 skills, 0 commands, 0 agents. Owner: OP-10 (#344), OP-11 (#345).
- **rust-quality** (owner OP-20 (#354))
  - **startup** — SessionStart registers the Rust post-tools.d module. `plugin init (bootstrap registry)` (hook): supported; evidence `load.local-file`. Owner: OP-20 (#354), OP-08 (#342).
  - **postTool** — Post-edit Rust quality checks reach the model and gate later commit/push. `tool.execute.after (append to output)` (hook): supported; evidence `post.feedback`, `post.bash-exit`. Owner: OP-20 (#354), OP-06 (#340).
  - **surfaces** — 0 skills, 0 commands, 0 agents. Owner: OP-10 (#344), OP-11 (#345).
- **statusline** (owner OP-25 (#359))
  - **tools** — status skill and setup command read helper output through bash. `bash tool + shell.env (helper path and environment)` (tool): supported; evidence `env.shell`. Owner: OP-25 (#359), OP-09 (#343).
  - **startup** — SessionStart publishes the status helper. `plugin init (bootstrap)` (hook): supported; evidence `load.local-file`. Owner: OP-25 (#359), OP-08 (#342).
  - **ui** — Show toolu session and gate status in the host UI. `client.tui.showToast → tui.toast.show` (tui): supported; evidence `ui.toast`. Owner: OP-25 (#359).
  - **surfaces** — 1 skills, 1 commands, 0 agents. Owner: OP-10 (#344), OP-11 (#345), OP-25 (#359).
  - **note** — A persistent statusline needs a TUI plugin (@opencode-ai/plugin/tui slots); headless probes cannot verify TUI rendering. Owner: OP-25 (#359).
- **toolu** (owner OP-02 (#336), OP-03 (#337), OP-04 (#338), OP-05 (#339), OP-06 (#340), OP-07 (#341), OP-08 (#342), OP-09 (#343), OP-10 (#344), OP-11 (#345), OP-24 (#358))
  - **tools** — Core pre-tool gates (bash-commands, commit-gate, quality-gate, protected-files, code-edit-rules, push-review, plan-ledger, docs-sync) allow or deny bash, edit, write, apply_patch and grep before they run. `tool.execute.before (throw to deny)` (hook): supported; evidence `deny.bash`, `deny.write`, `deny.apply-patch`. Owner: OP-03 (#337), OP-04 (#338).
  - **permission** — Gate ask decisions open a native prompt, and the user's permission rules stay authoritative. `permission.ask (declared, never invoked) + opencode.json permission` (hook): partial; evidence `permission.ask-hook`, `permission.config-deny`, `permission.order`. Alternative: Degrade ask with the @toolu/core/host class rules: security guardrails ask becomes a tool.execute.before deny, judgement gates ask becomes advice appended by tool.execute.after; the user's own ask/deny rules still apply after toolu allows. Owner: OP-05 (#339).
  - **startup** — SessionStart bootstraps selected plugins (registry, readiness) and injects the session protocol. `plugin init + experimental.chat.system.transform` (hook): supported; evidence `load.local-file`, `context.system`, `events.bus`. Owner: OP-07 (#341), OP-08 (#342), OP-09 (#343).
  - **prompt** — UserPromptSubmit reminders. `chat.message (output.parts)` (hook): supported; evidence `context.prompt`. Owner: OP-07 (#341).
  - **compaction** — PreCompact preserves gate and plan state through compaction. `experimental.session.compacting` (hook): supported; evidence `context.compaction`. Owner: OP-07 (#341).
  - **postTool** — gate-status, push-waiver and post-tools.d quality feedback after edits and bash. `tool.execute.after (append to output)` (hook): partial; evidence `post.feedback`, `post.bash-exit`, `post.tool-error`. Alternative: A thrown tool error has no side effect to check; it reaches the model as the tool error and the event bus as message.part.updated. Owner: OP-06 (#340).
  - **mcp** — mcp-blocker denies blocklisted MCP servers before the call. `tool.execute.before on <server>_<tool>` (hook): supported; evidence `deny.mcp`. Owner: OP-03 (#337), OP-04 (#338).
  - **task** — agent-tier gates delegated agents, and gates also apply inside child sessions. `tool.execute.before on task + child sessions` (hook): supported; evidence `deny.task-child`. Owner: OP-03 (#337), OP-04 (#338).
  - **surfaces** — 6 skills, 2 commands, 5 agents. Owner: OP-10 (#344), OP-11 (#345).
- **toolu-review** (owner OP-24 (#358))
  - **tools** — review skill writes push-review state through the helper. `bash tool + shell.env (helper path and environment)` (tool): supported; evidence `env.shell`. Owner: OP-24 (#358), OP-09 (#343).
  - **startup** — SessionStart publishes the push-review state writer. `plugin init (bootstrap)` (hook): supported; evidence `load.local-file`. Owner: OP-24 (#358), OP-08 (#342).
  - **surfaces** — 1 skills, 0 commands, 0 agents. Owner: OP-10 (#344), OP-11 (#345), OP-24 (#358).
- **ts-quality** (owner OP-18 (#352))
  - **startup** — SessionStart registers the TypeScript post-tools.d module. `plugin init (bootstrap registry)` (hook): supported; evidence `load.local-file`. Owner: OP-18 (#352), OP-08 (#342).
  - **postTool** — Post-edit TypeScript quality checks reach the model and gate later commit/push. `tool.execute.after (append to output)` (hook): supported; evidence `post.feedback`, `post.bash-exit`. Owner: OP-18 (#352), OP-06 (#340).
  - **surfaces** — 0 skills, 0 commands, 0 agents. Owner: OP-10 (#344), OP-11 (#345).
<!-- opencode-host-matrix:end -->

## Release blockers and limitations

When the pinned host does not support a required capability, the capability either blocks release or has an alternative backed by supported probes. An enforcement capability (one that decides whether a tool call runs) needs `alternativeEvidence` whose probes are all supported; prose alone never satisfies the check. Every unsupported probe verdict is assigned to an owner.

<!-- opencode-host-limitations:start -->
### Release blockers

None. Every required capability is supported on the pinned host or has an alternative backed by supported probes.

### Limitations and alternatives

- `ast-grep.tools` (unsupported) — search-nudge advises ast-grep before Grep or bash text search, without blocking. Alternative: Deliver the nudge with the tool result through tool.execute.after. Evidence: `post.feedback`. Owner: OP-13 (#347), OP-05 (#339).
- `toolu.permission` (partial) — Gate ask decisions open a native prompt, and the user's permission rules stay authoritative. Alternative: Degrade ask with the @toolu/core/host class rules: security guardrails ask becomes a tool.execute.before deny, judgement gates ask becomes advice appended by tool.execute.after; the user's own ask/deny rules still apply after toolu allows. Evidence: `deny.bash`, `post.feedback`, `permission.config-deny`, `permission.order`. Owner: OP-05 (#339).
- `toolu.postTool` (partial) — gate-status, push-waiver and post-tools.d quality feedback after edits and bash. Alternative: A thrown tool error has no side effect to check; it reaches the model as the tool error and the event bus as message.part.updated. Evidence: `post.feedback`, `post.bash-exit`, `post.tool-error`. Owner: OP-06 (#340).

### Host constraints

- Never throw from plugin init: the host logs 'failed to load plugin' and keeps running tools unguarded, so setup failures must become deny-all hooks. Evidence: `load.init-throw`. Owner: OP-02 (#336), OP-08 (#342).
- Export only plugin functions or one default PluginModule: every exported function is invoked as a plugin, and a non-hooks return breaks every prompt. Evidence: `load.helper-export`, `load.module-default`. Owner: OP-02 (#336).
- Install through opencode.json plugin (with options) or .opencode/plugins/; both routes load the documented plugin function. Evidence: `load.config-file`, `load.local-file`. Owner: OP-11 (#345), OP-26 (#360).
- Generated skill names must validate themselves (lowercase, single hyphens): the host loads invalid names without complaint. Evidence: `surface.names`. Owner: OP-10 (#344).
- Surfaces can be registered from the plugin config hook (commands, agents, skills.paths, instructions) instead of writing into user directories. Evidence: `surface.config-hook`, `surface.files`. Owner: OP-11 (#345).
- Helper environment (TOOLU_* paths, resolved Bun) reaches bash through shell.env. Evidence: `env.shell`. Owner: OP-09 (#343).
- Project commands keep their arguments; command.execute.before sees them. Evidence: `command.hook`. Owner: OP-10 (#344), OP-24 (#358).

### Experimental hooks under the exact pin

- `experimental.chat.system.transform`: `agent-browser.startup`, `context7.startup`, `epic-orchestrator.startup`, `exa-search.startup`, `jev.startup`, `jira.startup`, `pr-babysit.startup`, `toolu.startup`
- `experimental.session.compacting`: `toolu.compaction`
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
