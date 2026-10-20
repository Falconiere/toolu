# toolu-plugins

`toolu plugins install|list|remove|update` (#438): the plugin installer, ported
from the Node CLI that npm publishes as `@toolu/plugins`. That package is now a
wrapper (`toolu-plugins`) that finds the native `toolu` and runs these verbs.

- Claude Code, Codex and Muse are driven through `claude plugin …`,
  `codex plugin …` and `muse plugins …` (`src/host/`); each listing is
  normalized there.
- Muse (#549) installs from a toolu checkout of the tag `v<binary version>`
  (`src/host/checkout.rs`), registered as the marketplace `toolu` before
  install and update (`src/verbs/prepare.rs`). `src/verbs/trust.rs` approves
  pending hooks under `--approve` and otherwise names `muse plugins approve`.
  `src/host/muse.rs` reads the outputs captured in
  `fixtures/plugins/muse.json`.
- The catalog is `.claude-plugin/marketplace.json`, embedded with
  `include_str!` from the repository layout at build time; `--catalog <path>`
  reads another (the npm wrapper passes its bundled copy).
- Prompts are numbered lines on `/dev/tty` (`src/prompt.rs`); stdout stays the
  report. The crate never writes the standard streams (rule 14).

Exit codes follow the CLI contract: 64 for usage errors and missing input, 1
for a failed step. The user guide is [docs/cli/installer.md](../../docs/cli/installer.md);
the reference is [docs/cli/plugins.md](../../docs/cli/plugins.md).
