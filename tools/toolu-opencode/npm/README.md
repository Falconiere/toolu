# @toolu/opencode

This package is deprecated. It no longer runs toolu's hooks, and it never ran
them under OpenCode V2.

Install the native binary, then install the plugins into OpenCode with it:

```bash
curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash
brew install falconiere/tap/toolu
toolu plugins install --host opencode
```

Then remove `@toolu/opencode`, or the local plugin directory that depends on
it, from the `plugins` of your `opencode.json`.

Loaded by OpenCode V2, this package only reports the deprecation: it logs it
once and puts it in each session's instructions, so the agent can tell you.

Full documentation: **[docs/opencode.md](https://github.com/Falconiere/toolu/blob/main/docs/opencode.md)**

MIT © Falconiere Barbosa
