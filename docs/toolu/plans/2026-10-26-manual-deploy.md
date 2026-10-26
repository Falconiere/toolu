# Agent-run manual deploy — Plan

**Date:** 2026-10-26   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-26-manual-deploy-design.md   **Topic:** `/deploy` skill, rotation of the lost minisign release key, and one live dev prerelease.

## Evidence and approach

The plan reuses the release chain read during brainstorm and spec. That chain is `release-native.yml`, `release-finalize.yml`, `release-homebrew.yml`, `.github/scripts/release_native.py` (tested by `test_release_native.py` with subprocess runs against the real checkout), `cargo xtask homebrew-formula` (accepts prerelease tags), and `install.sh`. In `install.sh`, `verify()` writes one pub file from `TOOLU_PUBLIC_KEY`, `fetch()` is plain `curl -fsSL`, so `file://` works for `TOOLU_DOWNLOAD_BASE`, and `TOOLU_MINISIGN_PUB` is an override. No repository test pins the key value. `workflow_install.rs` checks only the workflow text and the minisign digest.

Memory recall gives the local gate quirks. The full gate's `tests` and `coverage` steps fail on this Mac (Homebrew toolu, `/Volumes` target, a live engine). Gate runs therefore use `npm ci` with `npm_config_store_dir` unset, `TMPDIR=/private/tmp`, and an isolated `TOOLU_RESOURCE_HOME`. A stale `GH_TOKEN` is set in this shell, so `gh` runs as `env -u GH_TOKEN gh`.

The design is the approved spec: a Claude Code skill at `.claude/skills/deploy/SKILL.md` with `disable-model-invocation: true`. The deterministic checks stay in `release_native.py`. `install.sh` gains `TOOLU_RETIRED_PUBLIC_KEY`, and the key rotation runs once from this session.

## Workstream summary

1. Helper and test.
2. Key rotation, installer change, and installer tests.
3. Skill and preflight checks.
4. Docs, then a commit.
5. The user types `/deploy` for the live dev run, which needs Rosetta and minisign installed first.
6. Install from the published prerelease, add a new-key fixture, and run the prod commands dry.
7. Gate.
8. Delivery.

## Steps (machine-readable)

```json
[
  {
    "id": "dev-tag-helper",
    "title": "release_native.py accepts -dev.<N> wherever -test is accepted",
    "check": "python3 -B -m unittest discover -s .github/scripts -p 'test_release_native.py' -v",
    "ac_refs": [
      "AC-1"
    ],
    "paths": [
      ".github/scripts/release_native.py",
      ".github/scripts/test_release_native.py",
      "Cargo.toml",
      "Cargo.lock",
      "package.json",
      "tools/toolu-cli/npm/package.json"
    ],
    "input": "Real workspace manifests at 8.4.1. Tags v8.4.1-dev.1 with and without --test-tag, v8.4.1-dev (no number), and v8.4.2-dev.1 (wrong version). expected_version() for -test and -dev tags.",
    "model": "sonnet"
  },
  {
    "id": "rotate-key",
    "title": "Generate the new release key, embed it in install.sh with the retired key, and store the GitHub secret",
    "check": "test \"$(sed -n 2p \"$HOME/.config/toolu/minisign.pub\")\" = \"$(sed -n 's/^TOOLU_PUBLIC_KEY=\"\\(.*\\)\"$/\\1/p' install.sh)\" && grep -qx 'TOOLU_RETIRED_PUBLIC_KEY=\"RWQnIicG+ykLjLRK43obFWLb8l27IN4cpEJmb5jdEIsvb/3Xo+QY4Knc\"' install.sh && test \"$(stat -f %Lp \"$HOME/.config/toolu/minisign.key\")\" = 600 && bash -n install.sh && env -u GH_TOKEN gh secret list --repo Falconiere/toolu --json name,updatedAt | jq -e 'any(.[]; .name == \"TOOLU_MINISIGN_SECRET_KEY\" and .updatedAt >= \"2026-10-26\")'",
    "ac_refs": [
      "AC-2"
    ],
    "paths": [
      "install.sh"
    ],
    "input": "A fresh minisign 0.12 key pair (minisign -G -W) and the live repository secret list.",
    "model": "inherit"
  },
  {
    "id": "installer-keys",
    "title": "install.sh verifies against the new key, then the retired key, and refuses any other signer",
    "check": "python3 -B -m unittest discover -s .github/scripts -p 'test_install_keys.py' -v && tmp=$(mktemp -d) && bash install.sh --version v8.4.0 --install-dir \"$tmp\" && test \"$(\"$tmp/toolu\" --version)\" = 'toolu 8.4.0'",
    "ac_refs": [
      "AC-3",
      "AC-4"
    ],
    "depends_on": [
      "rotate-key"
    ],
    "paths": [
      "install.sh",
      ".github/scripts/test_install_keys.py",
      ".github/scripts/fixtures/install"
    ],
    "input": "The real v8.4.0 SHA256SUMS and SHA256SUMS.minisig (old key), a SHA256SUMS signed by a throwaway key for tag v8.4.1-dev.9, both served through TOOLU_DOWNLOAD_BASE=file://, and the live v8.4.0 release.",
    "model": "sonnet"
  },
  {
    "id": "deploy-skill",
    "title": "Write the /deploy skill: 11 numbered steps, manual invocation only",
    "check": "f=.claude/skills/deploy/SKILL.md && grep -qx 'name: deploy' \"$f\" && grep -qx 'disable-model-invocation: true' \"$f\" && grep -q '^argument-hint: ' \"$f\" && test \"$(grep -cE '^[0-9]+\\. \\*\\*' \"$f\")\" -eq 11 && grep -qF 'infisical secrets get TOOLU_MINISIGN_SECRET_KEY' \"$f\" && grep -qE '^INFISICAL_PROJECT_ID=[0-9a-f-]+$' \"$f\" && grep -qF 'release_native.py verify-tag' \"$f\" && grep -qF 'homebrew-formula' \"$f\" && grep -qF 'autorelease: tagged' \"$f\"",
    "ac_refs": [
      "AC-5"
    ],
    "paths": [
      ".claude/skills/deploy/SKILL.md"
    ],
    "input": "The approved spec's interface table, failure modes, and step list.",
    "model": "inherit"
  },
  {
    "id": "infisical-store",
    "title": "Infisical project toolu (env prod) holds the signing key and the three reference values, matching install.sh",
    "check": "pid=$(sed -n 's/^INFISICAL_PROJECT_ID=\\([0-9a-f-]*\\)$/\\1/p' .claude/skills/deploy/SKILL.md) && test -n \"$pid\" && g() { infisical secrets get \"$1\" --plain --silent --projectId \"$pid\" --env prod; } && for name in TOOLU_PUBLIC_KEY TOOLU_RETIRED_PUBLIC_KEY MINISIGN_URL; do test \"$(g \"$name\")\" = \"$(sed -n \"s/^$name=\\\"\\(.*\\)\\\"$/\\1/p\" install.sh)\" || exit 1; done && w=$(mktemp -d) && echo probe > \"$w/m\" && minisign -S -s <(g TOOLU_MINISIGN_SECRET_KEY) -m \"$w/m\" -x \"$w/m.minisig\" >/dev/null && minisign -Vm \"$w/m\" -x \"$w/m.minisig\" -P \"$(sed -n 's/^TOOLU_PUBLIC_KEY=\"\\(.*\\)\"$/\\1/p' install.sh)\" >/dev/null",
    "ac_refs": [
      "AC-2"
    ],
    "depends_on": [
      "rotate-key",
      "deploy-skill"
    ],
    "paths": [
      "install.sh",
      ".claude/skills/deploy/SKILL.md"
    ],
    "input": "The live Infisical project toolu (env prod) and the rotated key in install.sh; the secret is streamed into minisign and never printed.",
    "model": "inherit"
  },
  {
    "id": "preflight-refusals",
    "title": "The skill's step-1 and step-2 commands refuse each bad input on this repository",
    "check": "f=.claude/skills/deploy/SKILL.md && grep -qF 'git ls-remote --exit-code --tags origin' \"$f\" && grep -qF 'git rev-parse HEAD' \"$f\" && ! python3 .github/scripts/release_native.py verify-tag v9.9.9 2>/dev/null && ! python3 .github/scripts/release_native.py verify-tag v8.4.1-dev.1 2>/dev/null && git ls-remote --exit-code --tags origin refs/tags/v8.4.0 >/dev/null && git fetch -q origin main && test \"$(git rev-parse HEAD)\" != \"$(git rev-parse origin/main)\"",
    "ac_refs": [
      "AC-5"
    ],
    "depends_on": [
      "dev-tag-helper",
      "deploy-skill"
    ],
    "paths": [
      ".claude/skills/deploy/SKILL.md",
      ".github/scripts/release_native.py"
    ],
    "input": "--env prod --tag v9.9.9 (version mismatch), --env prod --tag v8.4.1-dev.1 without --test-tag, --env dev --tag v8.4.0 (an existing remote tag), --env prod from this branch (HEAD differs from origin/main), and --env staging (refused by the skill's step-1 prose).",
    "model": "haiku"
  },
  {
    "id": "docs",
    "title": "Document the manual deploy, the key rotation, and the AGENTS.md tag rule",
    "check": "grep -qF '## Manual deploy' docs/releases/native.md && grep -qF '/deploy --env' docs/releases/native.md && grep -qF 'TOOLU_RETIRED_PUBLIC_KEY' docs/releases/native.md && grep -qF 'v8.4.0 and earlier' docs/install.md && grep -qF 'Infisical' docs/releases/native.md && grep -qF '/deploy' AGENTS.md && cargo xtask check-markdown-cli",
    "ac_refs": [
      "AC-2",
      "AC-5"
    ],
    "depends_on": [
      "installer-keys",
      "deploy-skill"
    ],
    "paths": [
      "docs/releases/native.md",
      "docs/install.md",
      "AGENTS.md",
      "install.sh"
    ],
    "input": "The final skill and installer behavior.",
    "model": "sonnet"
  },
  {
    "id": "gate",
    "title": "Gate every step except tests and coverage, plus every .github/scripts unit test",
    "check": "env -u npm_config_store_dir TMPDIR=/private/tmp TOOLU_RESOURCE_HOME=$(mktemp -d) cargo xtask gate --only gate-change --only fmt --only clippy --only guardrails --only layers --only reach --only deny --only machete --only unused-pub --only hooks --only workflows --only docs-cli --only cli-compat --only jscpd --only rust-quality --only docs --only context-budget --only final-removal --only gate-coverage --only packaging --only ci-paths --only bench --only pack && python3 -B -m unittest discover -s .github/scripts -p 'test_*.py'",
    "ac_refs": [
      "AC-10"
    ],
    "depends_on": [
      "dev-tag-helper",
      "installer-keys",
      "deploy-skill",
      "docs"
    ],
    "paths": [
      ".github",
      "install.sh",
      ".claude/skills/deploy",
      "docs",
      "AGENTS.md"
    ],
    "input": "The final committed tree.",
    "model": "haiku"
  }
]
```

## Critical files

- `.github/scripts/release_native.py`: add an `expected_version(tag, test_tag)` helper and accept `-(test|dev)` in `verify-tag`.
- `.github/scripts/test_release_native.py`: add the dev-tag cases.
- `.github/scripts/test_install_keys.py` (new) and `.github/scripts/fixtures/install/<tag>/SHA256SUMS{,.minisig}` (new). Fixtures exist for v8.4.1 (old key, real), v8.4.1-dev.9 (throwaway key), and v8.4.1-dev.1 (new key, real, added after the live run).
- `install.sh`: `TOOLU_PUBLIC_KEY` (new), `TOOLU_RETIRED_PUBLIC_KEY`, and a two-key `verify()`.
- `.claude/skills/deploy/SKILL.md` (new).
- `docs/releases/native.md`, `docs/install.md`, `AGENTS.md`.
- Outside the repository: `~/.config/toolu/minisign.{key,pub}`, the `TOOLU_MINISIGN_SECRET_KEY` repository secret, and Infisical project `toolu` (env `prod`) holding `TOOLU_MINISIGN_SECRET_KEY`, `TOOLU_PUBLIC_KEY`, `TOOLU_RETIRED_PUBLIC_KEY`, and `MINISIGN_URL`.

## Verification

- End to end: the user types `/deploy --env dev --tag v8.4.1-dev.1 --change-log "<notes>"` once the skill change is committed. The agent follows the 11 steps, the prerelease goes public, `latest` stays at v8.4.1, and `install.sh --version v8.4.1-dev.1` installs it.
- Real inputs: the live v8.4.1 release and its old-key signature, the new key, the live prerelease, and the repository's own manifests.
- Failure and boundary cases:
  - A throwaway-key signature is refused by `install.sh` (AC-4) and by the skill's step-6 check (AC-6).
  - A version-mismatched tag and a dev tag without `--test-tag` fail `verify-tag`.
  - An existing remote tag stops the run.
  - A prod run from a branch stops.
  - Gate failures stop the run until the user confirms they are host-only.
- Docs: `docs/releases/native.md`, `docs/install.md`, and `AGENTS.md` are updated in the `docs` step, with `cargo xtask check-markdown-cli`.
- Delivery:
  1. Make scoped Conventional Commits (`feat(release): …`).
  2. Run `toolu ledger run docs/toolu/plans/2026-10-26-manual-deploy.md --verify`.
  3. Run `toolu-review:review`.
  4. Run `toolu ledger verdict status` until it reports `overall: ready`.
  5. Push and open the PR against `main`.
  6. Run `pr-babysit:babysit`. CI is disabled, so babysit will see no checks; it reports that state rather than waiting forever.

## Deviations

- The `gate` step runs every gate step except `tests` and `coverage`, because both fail on this host for reasons that reproduce on a clean `origin/main` (memory: local gate env quirks). The diff changes no Rust source. The spec's AC-10 records this.

## Plan review (2026-10-26)

- live-dev-release: 🔴 blocker: AC-6 was claimed only in `input` prose, with no check observing it (Jev 0.10). Fixed by adding a `sign-guard` step that runs the skill's step-6 verify command against a throwaway-key signature (must fail) and the real signature (must pass with the trusted comment).
- live-dev-release: 🟡 should-fix: the step needs the skill change committed (the skill refuses a dirty tree). The order is commit, then the user's `/deploy`; that order is in the workstream summary.
- preflight-refusals: 🔵 consider: `--env staging` is refused only by step-1 prose, with no command to observe. Accepted; the agent's reading of the inputs is the behavior under test.
- Delivery: 🔵 consider: with CI disabled, babysit sees no checks. This is noted under Verification.
- Jev: AC-7 0.77 and AC-9 0.78 observed by their checks. AC coverage: AC-1 through AC-10 each map to at least one step.
- User request during execution: store `TOOLU_PUBLIC_KEY`, `TOOLU_RETIRED_PUBLIC_KEY`, and `MINISIGN_URL` in Infisical. The user also chose to store the signing key there and to have `/deploy` read it from Infisical. Added the `infisical-store` step, and the skill gains `INFISICAL_PROJECT_ID`.
- Evidence during execution: v8.4.1 was never tagged or released (PR #639 merged after the workflows were disabled), so `latest` is v8.4.0. The installer, preflight, live-run, and docs checks now name v8.4.0 where they meant the last old-key release.
- User decision during execution: Linux builds use `cargo zigbuild` on the host (zig 0.17.0, cargo-zigbuild 0.24.0) instead of `rust:1.99.0-alpine` containers. The Docker VM had 3.8 GB free.
- User decision during execution ("open pr"): the PR opens before the live `/deploy` dev run. The four steps that need that run (`live-dev-release`, `sign-guard`, `install-dev-release`, `prod-dry`, covering AC-6 to AC-9) moved out of the ledger to **Deferred live verification** below. Their checks are kept verbatim, to run after the user types `/deploy --env dev --tag v8.4.1-dev.1 --change-log "…"`. Until then AC-6 to AC-9 are unproven.

## Deferred live verification

Run each check after the live dev run, in order. Each must exit 0.

### live-dev-release (AC-7)

A user-typed /deploy --env dev --tag v8.4.1-dev.1 publishes the prerelease and leaves latest and the tap alone.

```bash
env -u GH_TOKEN gh release view v8.4.1-dev.1 --repo Falconiere/toolu --json isPrerelease,isDraft,assets,body | jq -e --rawfile notes target/deploy/v8.4.1-dev.1/notes.md '.isPrerelease and (.isDraft | not) and ([.assets[].name] | sort) == ["SHA256SUMS", "SHA256SUMS.minisig", "toolu-darwin-amd64.tar.gz", "toolu-darwin-arm64.tar.gz", "toolu-linux-amd64.tar.gz", "toolu-linux-arm64.tar.gz"] and (.body | rtrimstr("\n")) == ($notes | rtrimstr("\n"))' && test "$(env -u GH_TOKEN gh release view --repo Falconiere/toolu --json tagName --jq .tagName)" = v8.4.0 && env -u GH_TOKEN gh api repos/Falconiere/homebrew-tap/contents/Formula/toolu.rb --jq .content | base64 -d | grep -qF 'download/v8.4.0/'
```

### sign-guard (AC-6)

The skill's step-6 signature check refuses a throwaway-key signature and accepts the release key's.

```bash
d=target/deploy/v8.4.1-dev.1/dist && w=$(mktemp -d) && key=$(sed -n 's/^TOOLU_PUBLIC_KEY="\(.*\)"$/\1/p' install.sh) && minisign -G -W -p "$w/t.pub" -s "$w/t.key" >/dev/null && cp "$d/SHA256SUMS" "$w/SHA256SUMS" && minisign -S -s "$w/t.key" -m "$w/SHA256SUMS" -x "$w/SHA256SUMS.minisig" -t 'toolu v8.4.1-dev.1' >/dev/null && ! minisign -Vm "$w/SHA256SUMS" -x "$w/SHA256SUMS.minisig" -P "$key" >/dev/null 2>&1 && minisign -Vm "$d/SHA256SUMS" -x "$d/SHA256SUMS.minisig" -P "$key" | grep -qxF 'Trusted comment: toolu v8.4.1-dev.1' && grep -qF 'minisign -Vm' .claude/skills/deploy/SKILL.md
```

### install-dev-release (AC-3, AC-8)

The published prerelease installs through install.sh, the Linux archives run in Alpine, and the new-key fixture is accepted.

```bash
tmp=$(mktemp -d) && bash install.sh --version v8.4.1-dev.1 --install-dir "$tmp/bin" && test "$("$tmp/bin/toolu" --version)" = 'toolu 8.4.1' && env -u GH_TOKEN gh release download v8.4.1-dev.1 --repo Falconiere/toolu --pattern 'toolu-linux-*.tar.gz' -D "$tmp/dl" && for arch in amd64 arm64; do mkdir "$tmp/$arch" && tar -xzf "$tmp/dl/toolu-linux-$arch.tar.gz" -C "$tmp/$arch" && test "$(docker run --rm --platform linux/$arch -v "$tmp/$arch:/package:ro" --entrypoint /package/toolu alpine:3.20 --version)" = 'toolu 8.4.1' || exit 1; done && python3 -B -m unittest discover -s .github/scripts -p 'test_install_keys.py' -v
```

### prod-dry (AC-9)

The prod-only formula and relabel query work on real data without writing.

```bash
d=target/deploy/v8.4.1-dev.1/dist && cargo xtask homebrew-formula v8.4.1-dev.1 "$d/SHA256SUMS" > /private/tmp/toolu-deploy-formula.rb && test "$(grep -c 'sha256 "' /private/tmp/toolu-deploy-formula.rb)" -eq 4 && for h in $(awk '{print $1}' "$d/SHA256SUMS"); do grep -qF "sha256 \"$h\"" /private/tmp/toolu-deploy-formula.rb || exit 1; done && env -u GH_TOKEN gh pr list --repo Falconiere/toolu --state merged --label 'autorelease: pending' --json number,title | jq -e 'type == "array"'
```

