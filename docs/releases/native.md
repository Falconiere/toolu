# Native release runbook

`release-please.yml` owns version bumps and tags. Its Release PR updates the root package, Bun packages, Cargo workspace and lockfile, and every plugin manifest. Merging it creates a draft GitHub Release and the tag in one run. `npm-publish.yml` waits for the packages to resolve, then `release-native.yml` builds from that tag. The release is never public before native finalization.

## Native assets

The native build matrix runs on `macos-14` (Darwin arm64), `macos-15-intel` (Darwin amd64), `ubuntu-24.04` (Linux amd64 musl), and `ubuntu-24.04-arm` (Linux arm64 musl). Every build uses the pinned toolchain and `cargo build --release --locked --bin toolu`. Each `toolu-<os>-<arch>.tar.gz` has two root members: executable `toolu` and `LICENSE`. The release also carries `SHA256SUMS`, `toolu.spdx.json`, and GitHub build provenance attestations for the four archives. The installer and Homebrew formula in #457 consume this layout.

The reusable finalizer downloads the assets from the draft Release, checks the checksum and layout, runs `toolu --version` against the tag, verifies the Darwin code signature, and runs each static Linux binary in Alpine and Debian. Only after all four legs pass does the workflow publish the draft. A tag containing `-` becomes a prerelease, so GitHub's stable `latest` stays on the previous release; npm uses its `next` dist-tag instead of moving `latest`. The tap is delivered in #457 and is not updated by this workflow.

## Dry run

Dispatch `release-native.yml` with `publish=false` and an existing test tag of the form `vX.Y.Z-test` or `vX.Y.Z-test.1`, where `X.Y.Z` equals the workspace version at that tag. The dispatch builds the four binaries, packages them, and runs the same finalizer against the workflow artifact. It does not upload assets or publish a Release. The test tag must point to a commit with this workflow and its source; creating tags is separate from this worker's issue authorization.

## Failure and recovery

The repository variable `TOOLU_RELEASE_DISABLED=true` stops the automatic release-please job. If the native build, upload, or finalizer fails, the GitHub Release remains a draft and cannot replace stable `latest`. Inspect the failed job. For a transient runner or upload failure, dispatch `release-native.yml` with the same exact release tag and `publish=true`; it requires the existing draft, replaces its named assets, verifies them, then publishes. If source code or dependency repair is needed, do not move the release tag: keep the draft and cut a later release through a new Release PR. If npm already shipped, those packages remain on the registry; do not bump their versions by hand. The weekly advisory audit checks `Cargo.lock` at the latest stable tag.
