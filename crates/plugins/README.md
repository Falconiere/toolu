# toolu-plugins

`toolu plugins install|list|remove|update` (#438): the plugin installer, ported
from the Node CLI that npm publishes as `@toolu/plugins`. That package is now a
wrapper (`toolu-plugins`) that finds the native `toolu` and runs these verbs.

- Claude Code and Codex are driven through `claude plugin …` and
  `codex plugin …` (`src/host/`); `plugin list --json` is normalized there.
- The catalog is `.claude-plugin/marketplace.json`, embedded with
  `include_str!` from the repository layout at build time; `--catalog <path>`
  reads another (the npm wrapper passes its bundled copy).
- Prompts are numbered lines on `/dev/tty` (`src/prompt.rs`); stdout stays the
  report. The crate never writes the standard streams (rule 14).

Exit codes follow the CLI contract: 64 for usage errors and missing input, 1
for a failed step. The user guide is [docs/cli/installer.md](../../docs/cli/installer.md);
the reference is [docs/cli/plugins.md](../../docs/cli/plugins.md).
