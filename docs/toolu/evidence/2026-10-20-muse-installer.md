# Live run: `toolu plugins` on Muse (#549, AC-9)

2026-10-20, Muse Code 1.4.11 (`1.4.11-R6667.1`), the branch's debug `toolu`
(8.0.4). Every command ran with `env -i` and `HOME`/`XDG_*` inside a scratch
directory (`$SCRATCH`), so no real Muse state was read or changed.

## Run 1: seeded checkout, approve flow, idempotence, remove

The checkout was seeded as a shallow clone of this branch with a local tag
`v8.0.4` at its head and the origin URL set to GitHub, so the binary found the
tag and fetched nothing. On this branch only `plugins/toolu` carries
`.muse-plugin/plugin.json`; its five agent definitions are the runtime
capabilities Muse asks to approve.

```text
# seed: clone of branch 4fdc5756, local tag v8.0.4 at HEAD, origin URL set to GitHub
$ muse --version
Muse Code 1.4.11 (1.4.11-R6667.1)
[exit 0]
$ target/debug/toolu --version
toolu 8.0.4
[exit 0]
$ target/debug/toolu plugins install toolu --host muse
  + toolu            installed; awaiting approval: muse plugins approve toolu
[exit 0]
$ muse plugins inspect toolu --json | statuses
['review_needed']
$ target/debug/toolu plugins install toolu --host muse --approve
  = toolu            already installed at 8.0.4; approved
[exit 0]
$ muse plugins inspect toolu --json | statuses
['trusted_enabled']
$ target/debug/toolu plugins install toolu --host muse --approve
  = toolu            already installed at 8.0.4
[exit 0]
$ target/debug/toolu plugins list --host muse
  = toolu            8.0.4
  - ast-grep         not installed
  - python-quality   not installed
  - rust-quality     not installed
  - ts-quality       not installed
  - statusline       not installed
  - toolu-review     not installed
  - pr-babysit       not installed
  - brainstorm       not installed
  - delivery-flow    not installed
  - epic-orchestrator not installed
  - jev              not installed
[exit 0]
$ target/debug/toolu plugins update toolu --host muse --approve
  = toolu            current at 8.0.4
[exit 0]
$ target/debug/toolu plugins install toolu --host muse --scope user
toolu: --scope is for Claude Code; muse has no scope concept
[exit 64]
$ target/debug/toolu plugins remove toolu --host muse --yes
  - toolu            removed
[exit 0]
$ muse plugins list --json
{
  "plugins": []
}
```

The first install leaves the capabilities `review_needed` and names the
approve command (exit 0). `--approve` approves them (`trusted_enabled`); a
second `--approve` run finds nothing pending and runs no approve. The second
and later runs found the marketplace registered and refreshed it instead of
adding it again (real Muse refuses a second `marketplace add toolu`).

## Run 2: clean home, real clone of `v8.0.4` from GitHub

```text
$ target/debug/toolu plugins install brainstorm toolu --host muse
  + brainstorm       installed
  x toolu            plugin `toolu` is not available in marketplace `toolu`
[exit 1]
$ git -C $SCRATCH/.local/share/toolu/muse-marketplace describe --tags
v8.0.4
[exit 0]
```

The released `v8.0.4` predates the `.muse-plugin` manifests (#547), so Muse
skips `toolu` from that catalog and the step fails with Muse's line (exit 1),
while `brainstorm`, whose Claude manifest Muse imports, installs. This is the
documented "release without Muse manifests" case; the first release carrying
#547 installs every plugin.
