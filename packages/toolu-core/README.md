# @toolu/core

Internal library for [toolu](https://github.com/Falconiere/toolu). It holds the portable decision, policy, config and bridge types that host adapters share, plus the runner that executes the bash gate engine.

**There is no standalone use for this package.** It is published because [`@toolu/opencode`](https://www.npmjs.com/package/@toolu/opencode) depends on it. If you are looking to install toolu, you want one of:

```bash
npx toolu plugins install          # Claude Code, Codex
opencode plugin add @toolu/opencode  # OpenCode
```

## Runtime

**Bun only.** `src/runner/` uses `Bun.which` and `Bun.spawn`, so this package cannot run under Node. Sources ship unbuilt as TypeScript, because every consumer is a Bun host that reads `.ts` directly.

No API stability is promised between releases; it moves with the repository version.

MIT © Falconiere Barbosa
