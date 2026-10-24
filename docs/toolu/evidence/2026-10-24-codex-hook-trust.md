# Codex hook trust with a pinned marketplace (#626) — live evidence

Codex `codex-cli 0.164.0`, macOS, 2026-10-24. Private `CODEX_HOME` at `/private/tmp/w626/live/ch` with `auth.json` linked to the user's (the `host-smoke` pattern). Three fresh worktrees of `origin/main` (`git worktree add --detach`). Each session is the epic's Codex start: `codex --no-daemon --dangerously-bypass-approvals-and-sandbox`, driven in tmux and read with `tmux capture-pane`.

## 1. Pinned marketplace, first session: the review (AC-6 fixture, AC-7)

Setup: `codex plugin marketplace add Falconiere/toolu --ref v8.2.0` and `codex plugin add` for toolu, jev, epic-orchestrator, pr-babysit, delivery-flow, brainstorm, toolu-review. The folder-trust prompt (private home only; the real home trusts the toolu repository root) was accepted. Startup modal, wt1:

```text

  Hooks need review
  18 hooks are new or changed.
  Hooks can run outside the sandbox after you trust them.

› 1. Review hooks
  2. Trust all and continue
  3. Continue without trusting (hooks won't run)

  enter confirm · esc skip

```

Enter on "Review hooks" (what a prompt's Enter does) opens the browser the issue reported:

```text

  >_ OpenAI Codex (v0.164.0)
     /private/tmp/w626/live/wt1
  permissions: YOLO mode

  Hooks
  Lifecycle hooks from config and enabled plugins.

  ⚠ 18 hooks need review before they can run.

  Event                 Installed   Active      Review      Description

› PreToolUse            3           0           3           Before a tool executes
  PermissionRequest     0           0           0           When permission is requested
  PostToolUse           1           0           1           After a tool executes
  PreCompact            1           0           1           Before context compaction
  PostCompact           0           0           0           After context compaction
  SessionStart          11          0           11          When a new session starts
  SessionEnd            0           0           0           Right before a session ends
  UserPromptSubmit      2           0           2           When the user submits a prompt
↓

  t trust all · enter review · esc close
```

`t` trusted all 18; `config.toml` then held 18 `[hooks.state]` entries.

## 2. Two more fresh worktrees after that one trust: no review (AC-7)

wt2 and wt3 each reached the composer with no review screen:

```text
› Ask Codex to do anything

  GPT-6.1-Sol default · /private/tmp/w626/live/wt2
  ? for shortcuts

› Ask Codex to do anything

  GPT-6.1-Sol default · /private/tmp/w626/live/wt3
  ? for shortcuts
```

`config.toml` diff across both sessions: only the model-availability counter changed. The background auto-upgrade ran and recorded the pin:

```json
"ref_name": "v8.2.0",
"revision": "d394d1049788eff7cd336f312b797bf6a432ed2a"
```

## 3. Cause reproduced: a moving ref re-opens the review

With `ref` edited to `v8.1.4` (what every merge to an unpinned main does to the revision), the next session started normally while the auto-upgrade re-cloned (`ref_name: v8.1.4`, `revision: fffaaae2…`) and force-reinstalled the plugins in the background. The session after it showed the review again for all 18 hooks:

```text

  Hooks need review
  18 hooks are new or changed.
  Hooks can run outside the sandbox after you trust them.

› 1. Review hooks
  2. Trust all and continue
  3. Continue without trusting (hooks won't run)

  enter confirm · esc skip

```

## 4. The installer re-pins an unpinned marketplace and keeps trust (AC-1)

Same private home, its `toolu` marketplace replaced by the unpinned source (`codex plugin marketplace remove toolu`, `codex plugin marketplace add Falconiere/toolu`), 18 `[hooks.state]` entries and 7 toolu plugins in `config.toml`. Then `CODEX_HOME=… target/debug/toolu plugins update --host codex` (this branch), exit 0:

```text
  + jev              re-pinned to v8.2.0
  + pr-babysit       re-pinned to v8.2.0
  + brainstorm       re-pinned to v8.2.0
  + delivery-flow    re-pinned to v8.2.0
  + epic-orchestrator re-pinned to v8.2.0
  + toolu            re-pinned to v8.2.0
  + toolu-review     re-pinned to v8.2.0
```

`diff` of `config.toml` before and after; the only change is the pin, every `[plugins.*]` and `[hooks.state]` entry kept (18 and 7 before and after):

```diff
108a109
> ref = "v8.2.0"
```

A first run of this step re-added every installed plugin, including other marketplaces' (`gmail@openai-curated-remote` failed as `gmail@toolu`); the Codex listing now keeps only `marketplaceName: toolu` entries, and the run above is after that fix.

## 5. Already pinned: nothing moves, and the trust from section 1 holds again (AC-2, AC-7)

A second `toolu plugins update --host codex`, exit 0:

```text
  = jev              current at 8.2.0
  = pr-babysit       current at 8.2.0
  = brainstorm       current at 8.2.0
  = delivery-flow    current at 8.2.0
  = epic-orchestrator current at 8.2.0
  = toolu            current at 8.2.0
  = toolu-review     current at 8.2.0
```

The re-added plugins hold the v8.2.0 content trusted in section 1 (the v8.1.4 content of section 3 was never trusted), so a session in wt3 starts with no review:

```text
› Ask Codex to do anything

  GPT-6.1-Sol default · /private/tmp/w626/live/wt3
  ? for shortcuts
```

## 6. A toolu marketplace from elsewhere is left alone (AC-3)

A fresh private home whose `toolu` marketplace is a local copy (`codex plugin marketplace add /private/tmp/w626/live/localtoolu`). Codex itself replaces a local source on a git add (verified with `codex plugin marketplace add Falconiere/toolu --ref v8.2.0` alone), so the installer lists marketplaces before adding. `toolu plugins install jev --host codex`, exit 0; `config.toml` diff shows only the plugin:

```diff
3a4,6
> 
> [plugins."jev@toolu"]
> enabled = true
```

## 7. Doctor (AC-5)

```text
$ target/debug/toolu --host codex doctor          # the user's ~/.codex, unpinned
warn plugins: the Codex marketplace toolu follows its default branch, so Codex asks to review toolu's hooks again after every change to it; toolu plugins update --host codex
$ CODEX_HOME=<private home pinned to v8.1.4> target/debug/toolu --host codex doctor
warn plugins: the Codex marketplace toolu is pinned to v8.1.4, not v8.2.0; toolu plugins update --host codex
$ CODEX_HOME=<private home pinned to v8.2.0> target/debug/toolu --host codex doctor
ok plugins: 1 toolu plugins
```
