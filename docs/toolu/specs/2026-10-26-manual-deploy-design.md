# Agent-run manual deploy — Design

**Date:** 2026-10-26   **Status:** Approved   **Author:** Falconiere Barbosa (with Claude)   **Topic:** A `/deploy` skill an agent follows step by step to cut a signed toolu release locally while GitHub Actions is off, plus a rotation of the lost release-signing key.

## Problem

All nine GitHub Actions workflows have been disabled since 2026-10-26 because of a billing issue. With them off, release-please, `release-native.yml`, and `release-homebrew.yml` cannot run, so there is no way to ship toolu. The user wants an AI agent to run the deploy locally: `--env prod|dev --tag <tag> --change-log "<notes>"`. The release must still carry the signed layout that `install.sh` and the Homebrew formula require. The minisign secret key behind `TOOLU_MINISIGN_SECRET_KEY` is lost. Until the key is rotated, no new release can pass `install.sh` verification by any route, CI included.

## Non-Goals

1. The skill does not bump versions or edit CHANGELOG.md. release-please owns both; for prod, `--tag` must already equal the workspace version.
2. No npm publish (`@toolu/plugins` is only a deprecation notice), no SBOM (`toolu.spdx.json`), and no build provenance attestations (they need Actions OIDC).
3. No automated recovery or resume after a partial publish. The skill reports the recovery commands, and the user decides.
4. Re-enabling CI is out of scope, and so is the get.toolu.sh pin. The pin lives in `CodaSignal/toolu.sh` and is a user follow-up.
5. No Codex copy (`.agents/skills/deploy`) and no `scripts/deploy.sh`.

## Architecture

A Claude Code project skill at `.claude/skills/deploy/SKILL.md` sets `disable-model-invocation: true`, so only a user's `/deploy` can start it. The skill is a numbered procedure. The agent runs each step's commands, checks the stated observable result, and stops at the first failure. This was the user's direct request, and Jev chose `skill_only` (1.0). The decisive trade-off is the agent's judgment between steps (reading output, stopping, reporting) against a deterministic script. Determinism stays where it already lives, in tested helpers, so the skill adds only orchestration. It reuses:

- `.github/scripts/release_native.py`: `verify-tag`, `package`, and `verify-package`. It is extended to accept a `-dev.<N>` suffix wherever `-test` is accepted today.
- `cargo xtask gate` (the CI `verify` job) and `cargo xtask homebrew-formula <tag> <SHA256SUMS>` (the CI tap job).
- The same commands as `release-native.yml` and `release-finalize.yml`: `cargo build --release --locked --bin toolu`, ad-hoc `codesign`, `shasum -a 256`, `minisign -S -t "toolu <tag>"`, a signature check against `install.sh`'s `TOOLU_PUBLIC_KEY`, the static-linkage check with `file`, and container runs on `alpine:3.20`.
- Step 7 smoke-tests Linux archives with `release_native.py verify-package` inside `python:3.12-slim --platform linux/<arch>` (Debian glibc), then runs `--version` in `alpine:3.20`. Together these cover both libcs, as `release-finalize.yml` does.
- Linux builds run on the host with `cargo zigbuild --release --locked --bin toolu --target <arch>-unknown-linux-musl` (user decision 2026-10-26). The Docker VM disk is 98% full, and containers would compile amd64 under emulation. Zig links the musl targets statically, which the `file` check and the container runs verify. The darwin-amd64 build cross-compiles on the host with `--target x86_64-apple-darwin`, and its smoke test needs Rosetta. Docker pulls only the small smoke-test images, and those runs feed files through stdin rather than binding `/Volumes` paths, which Docker Desktop can show stale.

Key rotation is a one-time change in this PR, not a skill step. A new unencrypted key pair goes to `~/.config/toolu/minisign.{key,pub}` (directory 700, key 600). The new public key becomes `TOOLU_PUBLIC_KEY` in `install.sh`, and the old one becomes `TOOLU_RETIRED_PUBLIC_KEY`. Signatures are verified against it too, so v8.4.0 and earlier still install with `--version`; its secret is lost, so it signs nothing new. v8.4.1 was bumped by PR #639 but never tagged or released, because the workflows were already off. The new secret is stored with `gh secret set TOOLU_MINISIGN_SECRET_KEY`, so CI can sign again once it is re-enabled. It is also stored in Infisical (project `toolu`, env `prod`, path `/`), together with the reference values `TOOLU_PUBLIC_KEY`, `TOOLU_RETIRED_PUBLIC_KEY`, and `MINISIGN_URL`, which `install.sh` keeps embedding because end users cannot reach Infisical. Infisical is the skill's source for the signing key: step 6 streams `infisical secrets get TOOLU_MINISIGN_SECRET_KEY --plain` to `minisign -s` through process substitution, so the secret never lands on disk or in output. The local `~/.config/toolu/minisign.key` remains as the user's backup. CI's `sed 's/^TOOLU_PUBLIC_KEY=…/'` reads only the new key, because the retired variable's name starts differently.

## Interfaces / Schema

**Invocation:** `/deploy --env prod|dev --tag <tag> --change-log "<notes>"` (frontmatter: `name: deploy`, `description`, `argument-hint: --env prod|dev --tag <tag> --change-log "<notes>"`, `disable-model-invocation: true`).

| Input | prod | dev |
| --- | --- | --- |
| `--tag` | exactly `v<workspace version>` (`verify-tag <tag>`) | `v<workspace version>-dev.<N>` (`verify-tag <tag> --test-tag`) |
| source | clean tree, `HEAD == origin/main` after `git fetch origin main` | clean tree, any commit |
| GitHub Release | stable, becomes `latest` | prerelease (`--prerelease`), `latest` unchanged |
| tap | `Falconiere/homebrew-tap/Formula/toolu.rb` commit `toolu <tag>` | untouched |
| release PR | merged PR labeled `autorelease: pending` with title `chore(main): release <version>` is relabeled `autorelease: tagged` | untouched |

**Infisical:** the Infisical CLI must be logged in (`infisical login`) to project `toolu`, whose ID is written in the skill, env `prod`. `TOOLU_MINISIGN_SECRET_KEY` is read only by process substitution into `minisign -s`. Preflight compares the public `TOOLU_PUBLIC_KEY` value in Infisical with `install.sh`. If `gh api user` fails while `GH_TOKEN` is set, every `gh` command runs as `env -u GH_TOKEN gh`.

**Work directory:** `target/deploy/<tag>/` holds `dist/` (four `toolu-<os>-<arch>.tar.gz`, `SHA256SUMS`, `SHA256SUMS.minisig`), `notes.md` (the `--change-log` text, passed with `--notes-file`), `extract-<os>-<arch>/`, `roundtrip/`, `tap/`, and `bin/`. Linux Cargo target dirs are `target/deploy/linux-<arch>/`.

**Steps:**

1. Inputs
2. Preflight (read-only)
3. Gate
4. Darwin builds
5. Linux builds
6. Package and sign
7. Smoke test
8. Publish
9. Prod tap and relabel
10. Verify install
11. Report

**`release_native.py`:** `verify-tag <tag> --test-tag` accepts `v<version>-(test|dev)(.<id>)*`. `verify-package … --test-tag` strips `-test…` or `-dev…` before comparing `toolu --version`.

**`install.sh`:**

```bash
TOOLU_PUBLIC_KEY="<new key>"             # signs v8.4.1 and every later release
TOOLU_RETIRED_PUBLIC_KEY="RWQnIicG…Knc"  # signed v8.4.0 and earlier; secret lost
```

`verify()` tries `TOOLU_PUBLIC_KEY`, then `TOOLU_RETIRED_PUBLIC_KEY`. `TOOLU_MINISIGN_PUB` still overrides both. The trusted-comment and checksum checks are unchanged.

## Failure modes and edge cases

- **Bad or missing input:** an `--env` other than `prod` or `dev`, a missing `--tag`, or an empty `--change-log` stops at step 1 and names the input. Nothing runs.
- **Tag mismatch:** a prod tag other than `v<version>`, or a dev tag that is not `-dev.<N>`, makes `verify-tag` exit 1 with `tag … does not match workspace v…`. The skill stops at step 2.
- **Dirty tree, prod HEAD not equal to `origin/main`, or the tag already exists** (`git rev-parse -q --verify refs/tags/<tag>` or `git ls-remote --exit-code --tags origin refs/tags/<tag>`): stop at step 2.
- **Missing tool:** `gh` auth, the Docker daemon, `minisign`, `infisical`, `zig`, `cargo zigbuild`, Rosetta (`arch -x86_64 /usr/bin/true`), or a Rust target. The skill stops at step 2 and names the install command (`brew install minisign zig infisical/get-cli/infisical`, `cargo install --locked cargo-zigbuild`, `softwareupdate --install-rosetta --agree-to-license`, `rustup target add x86_64-apple-darwin aarch64-unknown-linux-musl x86_64-unknown-linux-musl`). It never installs system software without asking.
- **Infisical unavailable:** if the CLI is not logged in, the project is unreachable, or Infisical's `TOOLU_PUBLIC_KEY` differs from `install.sh`, the skill stops at step 2. It never prints `TOOLU_MINISIGN_SECRET_KEY`.
- **Gate failure:** stop at step 3 and list each failing check or test with its shortest decisive line. The run continues only when the user explicitly confirms in the session that every listed failure is a known host-environment failure (for example, a Homebrew `toolu` shadowing launcher tests). The release notes are not changed. A failure the user does not confirm ends the run. The skill has no flag that skips the gate.
- **Build failure, a non-static Linux binary, or a failed `codesign --verify`:** stop at step 4 or 5.
- **Signature not made by the install.sh key** (wrong Infisical secret) **or a missing trusted comment `toolu <tag>`:** stop at step 6, with nothing public.
- **Smoke failure** (checksum, layout, size, or a version that differs from the tag): stop at step 7, with nothing public.
- **Publish failure after the tag push** (create, upload, round-trip `cmp`, or the undraft): stop. The tag stays and the release stays a draft. A draft is never `latest`, so users see nothing. The skill prints the recovery commands `gh release delete <tag> --cleanup-tag --yes` and `git tag -d <tag>`, and runs them only if the user says so.
- **Skill not loaded:** if `/deploy` is not listed after the file lands, restart the session. The skill is never reproduced by hand.
- **Concurrency:** a second run racing on the same tag fails at `git push` because the remote tag exists. It stops before creating a release.
- **Tap unchanged:** if `git diff --cached --quiet` shows no change, there is no commit and the skill reports "formula unchanged".
- **Tap push rejected:** stop. The release is already public; the skill prints the formula path for a manual push.
- **No pending release PR found** (the version was bumped by hand): the skill reports it and continues.
- **Installer verification:** a signature from neither key fails with `SHA256SUMS of <tag> is not signed by the toolu release key; refusing to install` (exit 1). Old-key releases still pass through the retired key.

## Acceptance criteria

- **AC-1:** Given the real workspace version `8.4.1`, `release_native.py verify-tag v8.4.1-dev.1 --test-tag` exits 0, `verify-tag v8.4.1-dev.1` (without `--test-tag`) exits 1, and `verify-package` with `--test-tag` accepts a binary printing `toolu 8.4.1` for tag `v8.4.1-dev.1`.
- **AC-2:** After rotation, `install.sh` embeds the new `TOOLU_PUBLIC_KEY`, which matches the line-2 key of `~/.config/toolu/minisign.pub`, plus the old key as `TOOLU_RETIRED_PUBLIC_KEY`. The key file has mode 600, and `gh secret list` shows `TOOLU_MINISIGN_SECRET_KEY` updated on 2026-10-26. Infisical project `toolu` env `prod` holds the four values: its `TOOLU_PUBLIC_KEY` and `TOOLU_RETIRED_PUBLIC_KEY` equal `install.sh`, its `MINISIGN_URL` equals `install.sh`'s, and a file signed through `minisign -s <(infisical secrets get TOOLU_MINISIGN_SECRET_KEY --plain …)` verifies against `TOOLU_PUBLIC_KEY`.
- **AC-3:** Given the real pre-rotation release v8.4.0 (the current `latest`), `bash install.sh --version v8.4.0 --install-dir <tmp>` from this branch exits 0, and `<tmp>/toolu --version` prints `toolu 8.4.0`.
- **AC-4:** Given a local release tree whose `SHA256SUMS` is signed by a throwaway key and served through `TOOLU_DOWNLOAD_BASE`, `install.sh --version v8.4.1-dev.9` exits 1 with `is not signed by the toolu release key`.
- **AC-5:** `.claude/skills/deploy/SKILL.md` exists with `disable-model-invocation: true` and the 11 numbered steps. Running its step-1 and step-2 commands exactly as written stops each bad input: `--env staging`, `--env prod --tag v9.9.9`, `--env prod --tag v8.4.1` from this branch (`HEAD` differs from `origin/main`), and `--env dev --tag v8.4.0` (existing remote tag, and no `-dev` suffix). Each run names its failure and creates no tag or release (`git ls-remote` and `gh release list` unchanged). Claude Code blocks model invocation of this skill, so an agent never runs it as a tool call.
- **AC-6:** A `SHA256SUMS` signed with a throwaway key fails the step-6 check against `install.sh`'s key with nonzero exit. A signature by the new key passes and prints `Trusted comment: toolu <tag>`.
- **AC-7:** When the user types `/deploy --env dev --tag v8.4.1-dev.1 --change-log "<notes>"` from this branch publishes the prerelease `v8.4.1-dev.1`: `isPrerelease: true`, `isDraft: false`, the assets are exactly the 4 archives plus `SHA256SUMS` and `SHA256SUMS.minisig`, and the body equals the notes. `gh release view --json tagName` (latest) still reports `v8.4.0`, and the tap's `Formula/toolu.rb` still points at `download/v8.4.0/`.
- **AC-8:** Given the published v8.4.1-dev.1, `bash install.sh --version v8.4.1-dev.1 --install-dir <tmp>` exits 0 and `<tmp>/toolu --version` prints `toolu 8.4.1`. The published Linux archives run `--version` in `alpine:3.20` for both arches.
- **AC-9:** The prod-only commands work on real data without writing. `cargo xtask homebrew-formula v8.4.1-dev.1 target/deploy/v8.4.1-dev.1/dist/SHA256SUMS` prints a formula whose four `sha256` values equal the `SHA256SUMS` lines. The read-only relabel query `gh pr list --state merged --label "autorelease: pending" --json number,title` runs and returns JSON.
- **AC-10:** On the final tree, `cargo xtask gate --only` passes every gate step except `tests` and `coverage`, and the `.github/scripts` unit tests (`python3 -B -m unittest discover -s .github/scripts -p 'test_*.py'`) pass. The diff changes no Rust source, and those two steps fail on this host for reasons that reproduce on a clean `origin/main`. With CI disabled, no full gate runs elsewhere.

## Acceptance evidence

| AC | Real input | Expected result | Boundary/failure | Check |
| --- | --- | --- | --- | --- |
| AC-1 | Real `Cargo.toml`/`Cargo.lock` at 8.4.1; `sys.executable` stand-in binary for layout; a real built `toolu` for version | Exit codes as stated | Bare `-dev` without the flag fails; `-dev` must follow the exact version | `python3 -B -m unittest discover -s .github/scripts -p 'test_*.py'` with a new `test_dev_tag_*` case |
| AC-2 | Generated key pair; `install.sh` | Key equality; `stat -f %Lp` = 600; secret timestamp | Retired key name not matched by the CI `sed` | `diff <(sed -n 2p ~/.config/toolu/minisign.pub) <(sed -n 's/^TOOLU_PUBLIC_KEY="\(.*\)"$/\1/p' install.sh)`; `env -u GH_TOKEN gh secret list` |
| AC-3 | Live GitHub release v8.4.0 | Exit 0, `toolu 8.4.0` | Pre-rotation signature accepted via the retired key | `bash install.sh --version v8.4.0 --install-dir "$tmp"` |
| AC-4 | A dummy archive with a SHA256SUMS signed by a throwaway key, served through `TOOLU_DOWNLOAD_BASE=file://` | Exit 1 + message | Neither key matches | `.github/scripts/test_install_keys.py` |
| AC-5 | The skill's step-1 and step-2 commands with bad arguments on this repository | A stop message per case; no remote change | Existing tag, mismatched version, unknown env | `git ls-remote --tags origin` and `gh release list` before and after |
| AC-6 | Real `dist/SHA256SUMS`; a throwaway key and the new key | Nonzero, then 0 + trusted comment | Wrong key file | The step-6 `minisign -Vm … -P "$(sed … install.sh)"` command |
| AC-7 | The live `/deploy` dev run, typed by the user in this session | Release JSON as stated | `latest` unchanged; tap unchanged | `gh release view v8.4.1-dev.1 --json isPrerelease,isDraft,assets,body`; `gh release view --json tagName`; `gh api repos/Falconiere/homebrew-tap/commits?per_page=1` before and after |
| AC-8 | The published prerelease | Exit 0, `toolu 8.4.1`; `toolu 8.4.1` in alpine | Install through the new key | `bash install.sh --version v8.4.1-dev.1 --install-dir "$tmp"`; `docker run --platform linux/<arch> … alpine:3.20 --version` |
| AC-9 | The dev run's real `SHA256SUMS` | Hashes equal | Read-only query; no tap write | `cargo xtask homebrew-formula …` compared with `SHA256SUMS`; `gh pr list …` |
| AC-10 | The final tree | Exit 0 for every gate step except tests and coverage, and for the Python tests | Host-only failures excluded by step name, not by suppression | `cargo xtask gate --only gate-change --only fmt … --only pack` plus `python3 -B -m unittest discover -s .github/scripts -p 'test_*.py'` |

## Documentation impact

- `docs/releases/native.md`: a "Manual deploy" section (when to use it, `/deploy` usage, preconditions, prod-only steps, recovery) and a rotation record (2026-10-26, key lost, retired key still verifies v8.4.0 and earlier; Infisical holds the key, get.toolu.sh pin follow-up).
- `docs/install.md`: one sentence saying releases up to v8.4.0 verify against the retired key.
- `AGENTS.md` step 5: versions still come only from release-please, and while CI is unavailable the `/deploy` skill may create the release tag.
- `install.sh`: the header comment names both keys.

## Open Questions

1. Moving the get.toolu.sh pin in `CodaSignal/toolu.sh` to the merged `install.sh` commit. Owner: the user, after merge. Non-blocking: until the pin moves, the curl one-liner refuses post-rotation releases, while `--version` installs from this repository's `install.sh` and Homebrew (after the first prod run) work.
2. Resolved: the new secret key is backed up in Infisical (user decision 2026-10-26).

## Spec review (2026-10-26)

- Acceptance criteria: 🟡 should-fix: AC-5 did not cover the prod `HEAD == origin/main` refusal. Fixed by adding the prod-from-branch case.
- Acceptance criteria: 🟡 should-fix: AC-5 and AC-7 assumed the agent can invoke a `disable-model-invocation` skill. Fixed: AC-5 runs the skill's own commands, and AC-7 is typed by the user.
- Architecture: 🔵 consider: the Linux smoke images were unstated. Fixed by naming `python:3.12-slim` plus `alpine:3.20`.
- Jev: coverage 0.70 (gaps limited to the prod-only writes); prod writes proven by dry real-data evidence acceptable, 0.86. Gap stated in AC-9.
- Failure modes: 🟡 should-fix (found while planning): the full gate cannot pass on the user's Mac for host-environment reasons that reproduce on a clean `origin/main`. Fixed by adding a user-confirmed override for listed host failures. Jev was split (partial 0.54, confirm 0.45, confidence 0.31). The agent chose confirm because it keeps the full test signal in front of a human.
- Revision (user, during execution): the four values go to Infisical, and the skill reads the signing key from Infisical. v8.4.1 was never released, so v8.4.0 is the last old-key release; AC-2, AC-3, AC-5, and AC-7 are updated to match.
- Revision (user, during execution): Linux builds use cargo-zigbuild on the host instead of Docker, because the Docker VM disk has 3.8 GB free.
- Pre-push review (security): the retired key was accepted for any tag. It is now bounded by `TOOLU_RETIRED_KEY_LAST_TAG="v8.4.0"`, so a later tag signed by the old key is refused. `test_retired_key_is_refused_for_a_tag_after_v8_4_0` serves the real v8.4.0 signature as v8.5.0.
- No blockers. Open questions are owned and non-blocking.
