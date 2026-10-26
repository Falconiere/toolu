# Manual local deploy script

Date: 2026-10-26. Mode: standalone brainstorm, Full path (public release, signing key handling).

## Capsule

- **Outcome:** `./scripts/deploy.sh --env prod|dev --tag <tag> --change-log "<notes>"` runs the native release chain from a maintainer's Mac while GitHub Actions is disabled for billing. It produces the same signed release that `release-native.yml` and `release-homebrew.yml` would: four `toolu-<os>-<arch>.tar.gz` archives, `SHA256SUMS`, and `SHA256SUMS.minisig`, published to a GitHub Release. Prod also updates `Falconiere/homebrew-tap`.
- **Material defaults/non-goals:** Publish only. The script never bumps versions or edits CHANGELOG.md, and release-please stays the version owner (user decision). It skips npm, the SBOM, and build provenance attestations.
- **Repository evidence:** `.github/workflows/release-native.yml`, `release-homebrew.yml`, `release-finalize.yml`, `.github/scripts/release_native.py` (`verify-tag`, `package`, `verify-package`), `cargo xtask homebrew-formula`, `docs/releases/native.md`. Memory: all 9 workflows disabled on 2026-10-26.
- **Risk:** The minisign secret key exists only as a write-only GitHub secret. Without a local copy, the script cannot sign, `install.sh` refuses the release, and the fallback is a key rotation.
- **Handoff:** `/delivery-flow:delivery-flow` to spec and build.

## Interface

```text
scripts/deploy.sh --env prod|dev --tag <tag> --change-log "<release notes>"
```

- `--env prod`: `--tag` must equal `v<workspace version>` exactly (`release_native.py verify-tag`). HEAD must be a clean `main` equal to `origin/main`. The release is stable, becomes `latest`, and the tap is updated.
- `--env dev`: `--tag` must be `v<workspace version>-dev.<N>`. It builds from the current pushed HEAD and publishes a GitHub prerelease. The tap is untouched, matching the existing rule that a tag containing `-` is a prerelease.
- `--change-log`: required and non-empty. It becomes the GitHub Release body (`gh release create --notes`).
- Secrets come from the environment and are never flags: `TOOLU_MINISIGN_KEY_FILE` points to the secret-key file that is also stored as `TOOLU_MINISIGN_SECRET_KEY`.

## Flow

1. Preflight: parse the flags and check the environment (clean tree, branch/HEAD rules, tag absent locally and on the remote). Run `release_native.py verify-tag`. Check that `gh` auth, `docker`, `minisign`, `codesign`, and the key file are present.
2. Run `cargo xtask gate`, as the CI `verify` job does.
3. Build `cargo build --release --locked --bin toolu` for four targets:
   - darwin-arm64: host build.
   - darwin-amd64: `--target x86_64-apple-darwin`, after `rustup target add`.
   - linux-amd64 and linux-arm64: inside `docker run --platform linux/<arch> rust:1.99.0-alpine`, with a per-arch target dir. Check static linkage with `file`.
   - Ad-hoc `codesign`, then verify, both Darwin binaries.
4. Package: `release_native.py package` for each target, `shasum -a 256` into `SHA256SUMS`, then `minisign -S -t "toolu <tag>"`. Verify the signature against `TOOLU_PUBLIC_KEY` from `install.sh`.
5. Smoke test: `release_native.py verify-package` for both Darwin archives (amd64 runs under Rosetta). Run each Linux binary's `--version` in `alpine:3.20` and `debian:bookworm-slim`, the same images as `release-finalize.yml`.
6. Publish: create and push the annotated tag, then run `gh release create --draft [--prerelease]` and upload the assets. Download them back and `cmp` against the local files, then `gh release edit --draft=false [--prerelease]`.
7. Prod only: run `cargo xtask homebrew-formula <tag> dist/SHA256SUMS` and commit `toolu <tag>` to `Falconiere/homebrew-tap/Formula/toolu.rb`, using the user's `gh` credentials.

Nothing public happens before step 6. A failure in steps 1–5 leaves no tag and no release.

## Supporting changes

- `release_native.py`: accept `-dev.<N>` alongside `-test` in `verify-tag --test-tag` and `verify-package`. Add a case to `test_release_native.py`.
- `tooling/conventions/guardrails/rust/folders.json`: add `scripts` to `root`. This is registration data, so it ships with the code it registers.
- `docs/releases/native.md`: a short "Manual deploy" section with the preconditions (local key file, Docker, minisign), the two envs, and recovery steps.
- Prod precondition, documented but not automated: bump the version with a release-please PR. With CI disabled, run `npx release-please release-pr` locally, then merge the PR with admin rights, because the required checks never report.

## Axes

- **Version ownership.** The default is publish only, chosen by the user and agreed by Jev (`require_match`, 0.84). The rejected alternative had the script bump every version file from `release-please-config.json`, write CHANGELOG.md, commit, and tag. That would break the AGENTS.md rule and risk release-please drift once CI returns.
- **Entry point.** The default is `scripts/deploy.sh`, the path the user wrote. Jev favored a `cargo xtask deploy` task (0.96), weighing repository convention. The Bash script stays because the AGENTS.md shell ban covers hooks, skills, and agent commands, not maintainer tooling. It is also a thin orchestrator over already-tested pieces (`release_native.py`, `cargo xtask homebrew-formula`), much like the workflow steps themselves, which are Bash in YAML. Switching to xtask later is cheap. The rejected alternative, an xtask in Rust, means about 25 external commands wrapped in `Command` under the 300/50 line limits, for no gain in testability over the dev-env real run.
- **Linux cross-builds.** The default is Docker with the pinned `rust:1.99.0-alpine`. Docker is installed, and zig, cargo-zigbuild, and cross are not. The risk is that the amd64 build is slow under emulation.
- **Signing.** The default is a local minisign key file passed by env path and never echoed. The trust boundary is unchanged: `install.sh` and the Homebrew job still verify against the embedded public key and the `toolu <tag>` trusted comment.
- **Dev channel.** The default is a GitHub prerelease. A local-only install was rejected because `--tag` and `--change-log` imply a published artifact.

## Non-goals

- Bumping versions or editing CHANGELOG.md. release-please owns both.
- npm publish. `@toolu/plugins` is a deprecation notice, and skipping it leaves the registry on 8.4.1.
- SBOM (`toolu.spdx.json`) and build provenance attestations. Attestations need Actions OIDC, and nothing installs from the SBOM.
- Resuming a half-finished publish. Recovery is manual: delete the draft and the tag, then rerun.
- Re-enabling CI, and changes to `get.toolu.sh`.

## Open risks

- The local minisign secret key may not exist. If it is lost, rotate the key: a new `TOOLU_PUBLIC_KEY` in `install.sh`, a new pin in `get.toolu.sh`, and the old signatures revoked.
- `cargo xtask gate` has known host-environment failures on `/Volumes` checkouts (memory: local gate env quirks). Run it from a checkout and environment where it passes.
- A stale exported `GH_TOKEN` overrides the keyring login (memory, 2026-10-24). Run with `env -u GH_TOKEN` when it is invalid.
- A release published without CI provenance differs from earlier releases: it has no attestations and no SBOM. Say so in the release notes or the docs.
- Real-data check: a `--env dev --tag v8.4.1-dev.1` run publishes a real prerelease. Install it through `install.sh` with `VERSION=v8.4.1-dev.1` before the first prod run.

## Revision (delivery mode, same day)

New facts and user decisions after the standalone capsule:

- **Agent-run skill, not a script.** The user asked that "the script should run by an ai agent, create the step-by-step skill/command". Jev chose `skill_only` (1.0). The deliverable is now `.claude/skills/deploy/SKILL.md`, invoked as `/deploy --env prod|dev --tag <tag> --change-log "<notes>"`, with `disable-model-invocation: true` so only the user can start a release. `scripts/deploy.sh` and the `folders.json` change are dropped. Jev could not settle the location (0.49 insufficient, 0.34 for both hosts), so this change ships only the Claude Code skill, and a Codex copy is skipped.
- **The minisign key is lost.** The user approved rotating it in this PR: a new key at `~/.config/toolu/minisign.key` (mode 600), a new `TOOLU_PUBLIC_KEY` in `install.sh`, and `gh secret set TOOLU_MINISIGN_SECRET_KEY`. Jev (0.89) favored keeping the old public key as a retired verification key, so releases up to v8.4.1 stay installable with `--version`. The follow-up of moving the get.toolu.sh pin stays with the user.
- **Live check approved.** The skill will run once for real as `--env dev --tag v8.4.1-dev.1`, and that prerelease stays published as evidence.
- **Host facts:** Rosetta is not installed (the darwin-amd64 smoke test needs it), minisign is not installed (0.12 is available from Homebrew), and Docker 29.5.2 runs on arm64. The `rust:1.99.0-alpine` image exists for linux/amd64 and linux/arm64. The workspace needs C for `ring` (`cc`).
- **Scope (Jev):** the AGENTS.md wording update is in scope (0.76). The gate always runs (0.65). The prod step relabels the release PR from `autorelease: pending` to `autorelease: tagged` (0.78).
