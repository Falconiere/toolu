# @toolu/core

Internal library for [toolu](https://github.com/Falconiere/toolu). It holds portable decisions, config, native gates and dispatchers shared by host adapters, the host layer for Claude Code, Codex, Cursor, OpenCode and Hermes, state and plan-ledger logic, and shell command analysis (`@toolu/core/shell`, built on unbash; see [docs/shell-analysis.md](../../docs/shell-analysis.md)).

**There is no standalone use for this package.** It is published because [`@toolu/opencode`](https://www.npmjs.com/package/@toolu/opencode) depends on it. If you are looking to install toolu, you want one of:

```bash
npx @toolu/plugins install                  # Claude Code, Codex
npx @toolu/plugins install --host opencode  # OpenCode
```

## Runtime

**Bun only.** Sources ship unbuilt as TypeScript because every consumer is a Bun host that reads `.ts` directly.

No API stability is promised between releases; it moves with the repository version.

MIT © Falconiere Barbosa
