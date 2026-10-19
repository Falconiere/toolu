# Muse plugin distribution: marketplace catalog spike (#541)

> Date: 2026-10-19 · Muse Code 1.4.10 (`1.4.10-R6427.1`) · Part of epic #539 ·
> Input to #549 (installer adapter), #547 (`.muse-plugin` manifests), #551 (docs).

## TL;DR

- **A Muse marketplace catalog validates.** The earlier probes missed it:
  `install.transport` accepts only `"local-path"`, and the catalog also needs
  top-level `source: "local"` plus a per-plugin `availability.status` and
  `integrity.digest`. Every field is listed below.
- **toolu needs no new catalog file.** `muse plugins marketplace add` reads
  toolu's existing Codex catalog, `.agents/plugins/marketplace.json`, unchanged
  and computes each digest itself. Once a plugin directory has
  `.muse-plugin/plugin.json` (#547), Muse installs that manifest instead of the
  Claude or Codex one.
- **Decision: D-localdir.** The adapter keeps a toolu-owned shallow checkout of
  the release tag matching the binary, `v<toolu version>`, and registers that
  directory as the marketplace `toolu`. Install is `muse plugins install
  <name>@toolu`. Update moves the same checkout to the new tag, then runs
  `marketplace update` and `muse plugins update <name>`, which keeps hook
  approval unless a hook definition changed.
- **Approval is opt-in.** The adapter runs `muse plugins approve <name>` itself
  only under an explicit `--approve`. Otherwise it exits 0 and prints that argv
  for every plugin whose hooks are awaiting review.

## Native catalog format

A root `marketplace.json` that Muse 1.4.10 accepts (`plugin_count: 1`,
`skipped: []`). It is documented here but not used, because the digest would
have to be regenerated on every release:

```json
{
  "schemaVersion": 1,
  "source": "local",
  "plugins": [
    {
      "name": "statusline",
      "version": "8.0.3",
      "availability": { "status": "available" },
      "install": { "transport": "local-path", "source": "./plugins/statusline" },
      "integrity": { "digest": "sha256:<package digest Muse computes>" }
    }
  ]
}
```

| Field | Accepted on 1.4.10 |
|---|---|
| `schemaVersion` | required, `1` only: "marketplace must declare schemaVersion 1" |
| `source` | required, `"local"` only: "only local marketplace sources are supported" |
| `plugins[].name` | required |
| `plugins[].version` | required; shown by `list --available` but never checked, so the manifest's version is what gets installed |
| `plugins[].availability.status` | required; `available`, `deprecated` (listed, install refused) or `blocked`. `unavailable`, `yanked`, `hidden`, `disabled` and `installable` are invalid |
| `plugins[].install.transport` | required, `"local-path"` only; for `git`: "marketplace transport `git` is not supported" |
| `plugins[].install.source` | required; relative to the catalog or absolute. `../` is not rejected |
| `plugins[].integrity.digest` | required; `sha256:<package digest>`, verified at install: "failed integrity check" |
| top-level `name`, `description`; plugin `description`, `displayName`, `availability.reason`; any unknown key | accepted and ignored |

When the catalog is missing, Muse reports the paths it tried: `no marketplace
catalog found; probed marketplace.json, .agents/plugins/marketplace.json,
.claude-plugin/marketplace.json`. The Codex and Claude formats are imported and
normalized into the native shape, so `snapshot.json` under
`$XDG_DATA_HOME/muse/plugins/marketplaces/<name>/` shows the native shape.

## Observed behaviour behind the decision

| Observation | Consequence |
|---|---|
| A directory with `.muse-plugin`, `.claude-plugin` and `.codex-plugin` manifests selects `.muse-plugin`, with only a `multiple-manifests` warning | #547 adds one manifest; the catalog and the other manifests stay |
| toolu's plugins today: 11 of 12 skipped through `.claude-plugin` (`commandWindows` in `hooks.json`, `dependencies` entries); only `brainstorm` (skills, no hooks) imports as `claude-compatible` | No hook-carrying plugin installs before #547 |
| Git sources accepted: `file://…`, `https://github.com/Falconiere/toolu.git`, `Falconiere/toolu`, and `<url>#<tag>`, which checks out the tag; `marketplace update` stays on that tag. `@ref`, `/tree/<ref>` and `github:` fail | A Git marketplace would be possible |
| A Git marketplace's `marketplace update` writes `generations/<n>/source`, but the installed plugin keeps its old path. `muse plugins update` stays on the old version, and a second `install <name>@<mp>` is refused ("already installed from a different source") | With a Git marketplace, every update would be remove + install |
| A local-directory marketplace records the live path. After the directory changes, `marketplace update` offers the new version and `muse plugins update` installs it in place. Repeating an install from the same source exits 0 | In-place update works |
| Hooks are `review_needed` after install and `trusted_enabled` after `approve`. An update with unchanged hook definitions keeps `trusted_enabled`; a changed definition becomes `modified`. `remove` drops trust | Git updates would re-prompt every release; D-localdir re-prompts only when hooks change |
| Removing a marketplace leaves its plugins active, but `muse plugins update` then fails with `plugin-source-unavailable` | The adapter never removes the marketplace and re-adds it when it is missing |
| `XDG_DATA_HOME` and `XDG_CONFIG_HOME` relocate all Muse plugin, marketplace and trust state | Clean-machine tests run in a temporary home |

Rejected designs:

- **D-git** (`muse plugins marketplace add toolu Falconiere/toolu`): every update
  becomes remove, install and approve.
- **D-path** (`muse plugins install $DIR/plugins/$NAME`): loses `<name>@toolu`
  and `list --available`.
- **A committed native `marketplace.json`**: a per-release digest, with no gain
  over the Codex catalog.
- **Approval automatic or manual-only**: automatic approval would bypass Muse's
  explicit-trust step; manual-only leaves toolu silently not enforcing.

## The adapter's argv

The checkout directory is `$XDG_DATA_HOME/toolu/muse-marketplace`, falling back
to `$HOME/.local/share/toolu/muse-marketplace`. The tag is `v` plus the running
binary's version. The fetch comes before any `muse` call, so a failed fetch (for
example offline, or a development build with no tag) leaves Muse untouched.

```bash
DIR="${XDG_DATA_HOME:-$HOME/.local/share}/toolu/muse-marketplace"
TAG=v8.0.4
NAME=statusline

# install (and install --config replay). Clone only when $DIR is absent;
# otherwise move the existing checkout with the update fetch below.
git clone --depth 1 --branch "$TAG" https://github.com/Falconiere/toolu.git "$DIR"
muse plugins marketplace list --json   # skip the add when "toolu" is already at $DIR
muse plugins marketplace add toolu "$DIR" --json
muse plugins install "$NAME@toolu" --json
muse plugins approve "$NAME" --json    # only under --approve

# list
muse plugins list --json               # .plugins[] | {id: .plugin.id, version: .plugin.version, enabled: .record.enabled}
muse plugins list --available --json   # .available[] | select(.marketplace == "toolu")
muse plugins inspect "$NAME" --json    # .runtime_capabilities[].status: review_needed, trusted_enabled, modified

# update, after the toolu binary itself is upgraded to $TAG
git -C "$DIR" fetch --depth 1 origin tag "$TAG"
git -C "$DIR" checkout --detach "$TAG"
muse plugins marketplace update toolu --json
muse plugins update "$NAME" --json
muse plugins approve "$NAME" --json    # under --approve, only when inspect shows review_needed or modified

# remove (keeps plugin data, the marketplace and the checkout)
muse plugins remove "$NAME" --json
```

How these map onto `toolu plugins`, for #549 to implement (the flag name is
#549's call):

```text
toolu plugins install --host muse [<name>...] [--approve]
toolu plugins install --host muse --config .toolu/plugins.json [--approve]
toolu plugins list --host muse
toolu plugins update --host muse [<name>...] [--approve]
toolu plugins remove --host muse <name>... --yes
```

- **Approval.** With `--approve`, the adapter runs `muse plugins approve` as the
  invoking user, without prompting (the verb never prompts), right after
  installing or updating that plugin. Without it, the run still succeeds and
  lists each plugin whose `inspect` status is `review_needed` or `modified`,
  with the exact approve argv. Until approved, Muse keeps those hooks inert;
  skills and commands are active. On Claude Code and Codex, which have no
  approval step, the flag does nothing.
- **`install --config` replay.** This is the install sequence for the file's
  `enabled` names. An existing `$DIR` is moved with the update fetch instead of
  cloned, a marketplace already registered at `$DIR` is skipped, and a repeated
  install from the same source exits 0 unchanged. The selection format
  stays at version 1; #549 adds `"muse"` to its accepted `host` values.
- **`update` for installed plugins.** Move the checkout, refresh the snapshot,
  then `muse plugins update` each installed toolu plugin. A plugin whose
  `record.source.path` lies outside `$DIR` (one the user installed from some
  other source) is reported with `muse plugins remove` and reinstall, not
  updated in place.

## Failure modes

- **A `toolu` marketplace already registered with another source:** fail before
  installing anything, naming that source and `muse plugins marketplace remove
  toolu`. The adapter never replaces a user's source.
- **`$DIR` exists but is not a toolu checkout** (no
  `.agents/plugins/marketplace.json`, or a different `origin`): fail naming the
  path, and never delete it.
- **A missing tag:** `git clone --branch v0.0.0-missing` exits non-zero and
  leaves no checkout.

## Evidence

- A replay script ran the argv above (kept out of the tree, since toolu commits
  no shell scripts; its transcript is in the PR). It used a clean, isolated
  Muse 1.4.10 home and a `file://` origin whose tags v8.0.3, v8.0.4 and a
  hook-changing v8.0.5 add `plugins/statusline/.muse-plugin/plugin.json`. All
  22 assertions passed:
  - the native catalog lists and installs both a probe plugin and `statusline`;
  - a wrong digest and `schemaVersion: 2` are refused;
  - the checkout's Codex catalog lists `statusline`;
  - the plugin installs at 8.0.3 with hooks `review_needed`, then
    `trusted_enabled` after approve;
  - replay is idempotent;
  - 8.0.4 is offered while 8.0.3 stays installed, then updates to 8.0.4 with
    trust kept;
  - the hook change shows `modified`, then re-approves;
  - the plugin is removed.
- `git clone --depth 1 --branch v8.0.4 https://github.com/Falconiere/toolu.git`
  yields `.agents/plugins/marketplace.json` with 12 plugins.

## Re-probe on the next Muse minor

Two behaviours could flip this decision:

1. A Git marketplace's `muse plugins update` following the new generation. That
   would make D-git viable without re-approval.
2. `multiple-manifests` becoming an error. #547 would then need a Muse-only
   plugin directory.

Owner: #553 (live end-to-end verification).
