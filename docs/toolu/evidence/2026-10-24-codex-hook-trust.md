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
