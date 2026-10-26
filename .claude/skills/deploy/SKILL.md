---
name: deploy
description: Cut a signed toolu production release from this Mac while GitHub Actions is unavailable. Builds the four native archives, signs SHA256SUMS with the release key held in Infisical, publishes the GitHub Release, and updates the Homebrew tap.
argument-hint: --env prod --tag <tag> --change-log "<notes>"
disable-model-invocation: true
---

# Deploy toolu from this machine

Arguments: $ARGUMENTS

This is the local stand-in for `release-native.yml` and `release-homebrew.yml` (see `docs/releases/native.md`). Typing `/deploy` with these arguments authorizes every step below, including the public writes in steps 8 and 9. Work through the steps in order. After each command, check the stated result. On any other result, stop: report the step number and the shortest decisive output line, and run nothing further.

Never print, echo, or write `TOOLU_MINISIGN_SECRET_KEY` to a file. It only reaches `minisign` through process substitution.

Shell state does not persist between commands. Start every command with this prelude, with `<tag>` substituted:

```bash
TAG=<tag>; W=target/deploy/$TAG; D=$W/dist; PATH="$HOME/.cargo/bin:$PATH"
INFISICAL_PROJECT_ID=df71fe99-a8bf-462c-b187-0ce4df2275f0
```

`gh` below means `env -u GH_TOKEN gh` whenever `gh api user` fails while `GH_TOKEN` is set.

1. **Read the inputs.** From the arguments, take `--env`, `--tag` and `--change-log`.
   - Refuse unless `--env` is `prod`, `--tag` is present, and `--change-log` is non-empty. `prod` is the only environment; there is no dev or rc channel.
   - The tag is `v<workspace version>`, the version release-please already set.
   - Create `$W` and write the change log verbatim to `$W/notes.md` with the Write tool.

2. **Preflight.** Nothing public happens here. Every check must pass:
   - `git status --porcelain` prints nothing.
   - `git fetch origin main && test "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)"`.
   - `python3 .github/scripts/release_native.py verify-tag "$TAG"` exits 0.
   - The tag is new: `git rev-parse -q --verify "refs/tags/$TAG"` fails, and `git ls-remote --exit-code --tags origin "refs/tags/$TAG"` exits 2.
   - `cargo --version` prints the `channel` from `rust-toolchain.toml`, which proves rustup's cargo wins over Homebrew's.
   - These all succeed: `gh api user`, `docker info`, `minisign -v`, `zig version`, `cargo zigbuild --help`, `infisical --version`, `arch -x86_64 /usr/bin/true` (Rosetta), and `rustup target list --installed`. The last one must list `x86_64-apple-darwin`, `aarch64-unknown-linux-musl` and `x86_64-unknown-linux-musl`.
   - This command prints the same key as install.sh's `TOOLU_PUBLIC_KEY` line: `infisical secrets get TOOLU_PUBLIC_KEY --plain --silent --projectId "$INFISICAL_PROJECT_ID" --env prod`. If Infisical is not logged in, ask the user to run `! infisical login`.

   For a missing tool, name its install command and stop; do not install system software unasked. The install commands are:
   - `brew install minisign zig infisical/get-cli/infisical`
   - `cargo install --locked cargo-zigbuild`
   - `softwareupdate --install-rosetta --agree-to-license`
   - `rustup target add x86_64-apple-darwin aarch64-unknown-linux-musl x86_64-unknown-linux-musl`

3. **Gate.** Run `cargo xtask gate`. If it fails, list every failing step or test with its shortest decisive line. Continue only when the user confirms in this session that each listed failure is a known host-environment failure, such as a Homebrew `toolu` shadowing the launcher tests. Otherwise stop. The gate has no skip.

4. **Build Darwin.** Both binaries must build, sign and verify:

   ```bash
   for target in aarch64-apple-darwin x86_64-apple-darwin; do
     cargo build --release --locked --bin toolu --target "$target" &&
       codesign --force --sign - "target/$target/release/toolu" &&
       codesign --verify --verbose=2 "target/$target/release/toolu" || exit 1
   done
   ```

5. **Build Linux.** Both binaries must build and be statically linked:

   ```bash
   for target in aarch64-unknown-linux-musl x86_64-unknown-linux-musl; do
     cargo zigbuild --release --locked --bin toolu --target "$target" &&
       file "target/$target/release/toolu" | grep -Eq '(static-pie|statically) linked' || exit 1
   done
   ```

6. **Package and sign.** The result must be exactly four archives plus `install.sh`, which get.toolu.sh serves from the latest release. The last command must print `Trusted comment: toolu $TAG`:

   ```bash
   rm -rf "$D" && mkdir -p "$D" &&
   python3 .github/scripts/release_native.py package darwin arm64 target/aarch64-apple-darwin/release/toolu "$D" &&
   python3 .github/scripts/release_native.py package darwin amd64 target/x86_64-apple-darwin/release/toolu "$D" &&
   python3 .github/scripts/release_native.py package linux arm64 target/aarch64-unknown-linux-musl/release/toolu "$D" &&
   python3 .github/scripts/release_native.py package linux amd64 target/x86_64-unknown-linux-musl/release/toolu "$D" &&
   (cd "$D" && shasum -a 256 -- *.tar.gz > SHA256SUMS) &&
   cp install.sh "$D/install.sh" &&
   minisign -S -s <(infisical secrets get TOOLU_MINISIGN_SECRET_KEY --plain --silent --projectId "$INFISICAL_PROJECT_ID" --env prod) \
     -m "$D/SHA256SUMS" -x "$D/SHA256SUMS.minisig" -t "toolu $TAG" &&
   minisign -Vm "$D/SHA256SUMS" -x "$D/SHA256SUMS.minisig" -P "$(sed -n 's/^TOOLU_PUBLIC_KEY="\(.*\)"$/\1/p' install.sh)"
   ```

7. **Smoke test.** Use the checks from `release-finalize.yml`. Linux archives go to the containers on stdin, because Docker Desktop can show `/Volumes` binds stale. Every command must exit 0. Each Linux run must print `toolu <workspace version>`:

   ```bash
   for arch in arm64 amd64; do
     python3 .github/scripts/release_native.py verify-package "$TAG" "$D/toolu-darwin-$arch.tar.gz" "$D/SHA256SUMS" "$W/extract-darwin-$arch" &&
       codesign --verify --verbose=2 "$W/extract-darwin-$arch/toolu" || exit 1
     tar -c .github/scripts/release_native.py -C "$D" "toolu-linux-$arch.tar.gz" SHA256SUMS |
       docker run --rm -i --platform "linux/$arch" python:3.12-slim sh -c \
         "mkdir /w && cd /w && tar -x && python3 .github/scripts/release_native.py verify-package '$TAG' toolu-linux-$arch.tar.gz SHA256SUMS x" || exit 1
     docker run --rm -i --platform "linux/$arch" alpine:3.20 sh -c 'tar -xz -C /tmp && /tmp/toolu --version' < "$D/toolu-linux-$arch.tar.gz" || exit 1
   done
   ```

8. **Publish.** Every command must exit 0, and `cmp` must report no difference:

   ```bash
   git tag -a "$TAG" -m "toolu $TAG" && git push origin "refs/tags/$TAG" &&
   gh release create "$TAG" --verify-tag --draft --title "$TAG" --notes-file "$W/notes.md" &&
   gh release upload "$TAG" "$D"/toolu-*.tar.gz "$D/SHA256SUMS" "$D/SHA256SUMS.minisig" "$D/install.sh" &&
   rm -rf "$W/roundtrip" && gh release download "$TAG" -D "$W/roundtrip" &&
   for f in "$D"/*; do cmp "$f" "$W/roundtrip/${f##*/}" || exit 1; done &&
   gh release edit "$TAG" --draft=false
   ```

9. **Update the tap and the release PR.**
   - Run the tap commands below. If the formula did not change, report that and do not commit.
   - Relabel the release PR. If no merged PR titled `chore(main): release <version>` carries `autorelease: pending`, say so and continue.

   ```bash
   cargo xtask homebrew-formula "$TAG" "$D/SHA256SUMS" > "$W/toolu.rb" &&
   rm -rf "$W/tap" && gh repo clone Falconiere/homebrew-tap "$W/tap" -- --depth 1 &&
   mkdir -p "$W/tap/Formula" && cp "$W/toolu.rb" "$W/tap/Formula/toolu.rb" &&
   git -C "$W/tap" add Formula/toolu.rb &&
   { git -C "$W/tap" diff --cached --quiet || { git -C "$W/tap" commit -m "toolu $TAG" && git -C "$W/tap" push; }; }
   pr=$(gh pr list --repo Falconiere/toolu --state merged --label "autorelease: pending" --json number,title \
     --jq ".[] | select(.title == \"chore(main): release ${TAG#v}\") | .number")
   [ -z "$pr" ] || gh pr edit "$pr" --repo Falconiere/toolu --remove-label "autorelease: pending" --add-label "autorelease: tagged"
   ```

10. **Verify the install.** `bash install.sh --version "$TAG" --install-dir "$W/bin"` must exit 0, and `"$W/bin/toolu" --version` must print `toolu <workspace version>`. Then `gh release view --json tagName --jq .tagName` must print `$TAG`. Both served installers must be byte-identical to `install.sh`:
   - `curl -fsSL https://github.com/Falconiere/toolu/releases/latest/download/install.sh | cmp - install.sh`
   - `curl -fsSL https://get.toolu.sh/pkg/toolu/install | cmp - install.sh`

   A mismatch on the second means the get.toolu.sh worker does not redirect to the release asset yet. Report it, because the curl one-liner may refuse this release.

11. **Report.** Give the release URL (`gh release view "$TAG" --json url --jq .url`), the tagged commit, the tap commit, and the relabeled PR. Then name the gaps compared with CI: no SBOM and no build provenance attestation.

## If a step fails after the tag push

The tag stays, and the release stays a draft, so it is never `latest`. Report these recovery commands and run them only when the user asks:
- `gh release delete "$TAG" --cleanup-tag --yes`
- `git tag -d "$TAG"`

A rejected tap push leaves the release public. Report `$W/toolu.rb` so the user can push it by hand.
