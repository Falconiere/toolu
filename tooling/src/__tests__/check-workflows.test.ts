import { afterEach, expect, test } from "bun:test";
import { cpSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checkWorkflows } from "../check-workflows.ts";

const source = resolve(import.meta.dir, "../../..");
const roots: string[] = [];

function fixture(): string {
  const root = mkdtempSync(join(tmpdir(), "toolu-workflows-"));
  roots.push(root);
  mkdirSync(join(root, ".github"));
  cpSync(join(source, ".github/workflows"), join(root, ".github/workflows"), { recursive: true });
  cpSync(
    join(source, ".github/code-review-prompt.md"),
    join(root, ".github/code-review-prompt.md"),
  );
  cpSync(join(source, "release-please-config.json"), join(root, "release-please-config.json"));
  cpSync(join(source, "install.sh"), join(root, "install.sh"));
  return root;
}

function mutate(root: string, file: string, before: string, after: string): void {
  const path = join(root, file);
  const text = readFileSync(path, "utf8");
  expect(text).toContain(before);
  writeFileSync(path, text.replaceAll(before, after));
}

afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

test("committed workflows satisfy the release contract", () => {
  expect(checkWorkflows(source)).toEqual([]);
});

for (const scenario of [
  {
    name: "path-skipped required check",
    file: ".github/workflows/tests.yml",
    before: "  pull_request:\n    branches: [main]",
    after: "  pull_request:\n    branches: [main]\n    paths: ['crates/**']",
    finding: "without path filters",
  },
  {
    name: "unneeded write permission",
    file: ".github/workflows/tests.yml",
    before: "  rust:\n    name:",
    after: "  rust:\n    permissions:\n      contents: write\n    name:",
    finding: "unnecessary contents: write",
  },
  {
    name: "release workflow default write permission",
    file: ".github/workflows/release-please.yml",
    before: "permissions:\n  contents: read",
    after: "permissions:\n  contents: write",
    finding: "least-privilege default contents permission",
  },
  {
    name: "unpinned action",
    file: ".github/workflows/tests.yml",
    before: "actions/checkout@v4",
    after: "actions/checkout@main",
    finding: "not version-pinned",
  },
  {
    name: "missing ast-grep install",
    file: ".github/workflows/tests.yml",
    before:
      "# The rust-quality step runs the plugin's ast-grep rules and fails closed without it.\n      - name: Install ast-grep\n        run: npm install -g @ast-grep/cli",
    after:
      "# The rust-quality step runs the plugin's ast-grep rules and fails closed without it.\n      - name: Install ast-grep\n        run: npm install -g @example/unused",
    finding: "lacks @ast-grep/cli",
  },
  {
    name: "macOS loses Rust tests",
    file: ".github/workflows/tests.yml",
    before: "run: cargo xtask gate --only clippy --only tests",
    after: "run: cargo xtask gate --only clippy",
    finding: "macOS leg must run clippy and Rust tests",
  },
  {
    name: "removed native target",
    file: ".github/workflows/release-native.yml",
    before: "target: aarch64-unknown-linux-musl",
    after: "target: x86_64-unknown-linux-gnu",
    finding: "lacks aarch64-unknown-linux-musl",
  },
  {
    name: "unlocked native build",
    file: ".github/workflows/release-native.yml",
    before: "cargo build --release --locked --bin toolu",
    after: "cargo build --release --bin toolu",
    finding: "must use --locked",
  },
  {
    name: "finalizer racing the upload",
    file: ".github/workflows/release-native.yml",
    before: "needs: [package, upload]",
    after: "needs: [package]",
    finding: "must wait for packaged and uploaded assets",
  },
  {
    name: "missing release verifier",
    file: ".github/workflows/release-finalize.yml",
    before: "release_native.py verify-package",
    after: "missing_release.py verify-package",
    finding: "must verify both published and dry-run archives",
  },
  {
    name: "fork-unsafe review exemption",
    file: ".github/workflows/toolu-review.yml",
    before:
      "github.head_ref != 'release-please--branches--main--components--toolu' || github.event.pull_request.head.repo.full_name != github.repository",
    after: "github.head_ref != 'release-please--branches--main--components--toolu'",
    finding: "same-repo release PR",
  },
  {
    name: "no draft release",
    file: "release-please-config.json",
    before: '"draft": true',
    after: '"draft": false',
    finding: "draft and a real tag",
  },
  {
    name: "cancelled release publication",
    file: ".github/workflows/npm-publish.yml",
    before: "cancel-in-progress: false",
    after: "cancel-in-progress: true",
    finding: "must not cancel an in-flight publication",
  },
  {
    name: "tap updated for a prerelease",
    file: ".github/workflows/release-homebrew.yml",
    before: "if: ${{ !contains(inputs.tag, '-') }}",
    after: "if: ${{ !contains(github.ref_name, '-') }}",
    finding: "must skip prerelease tags by inputs.tag",
  },
  {
    name: "tap token for every repository",
    file: ".github/workflows/release-homebrew.yml",
    before: "repositories: homebrew-tap",
    after: "repositories: toolu",
    finding: "scoped to homebrew-tap",
  },
  {
    name: "inherited Homebrew secrets",
    file: ".github/workflows/release-please.yml",
    before:
      "    secrets:\n      HOMEBREW_APP_ID: ${{ secrets.HOMEBREW_APP_ID }}\n      HOMEBREW_APP_PRIVATE_KEY: ${{ secrets.HOMEBREW_APP_PRIVATE_KEY }}",
    after: "    secrets: inherit",
    finding: "Homebrew App secrets explicitly",
  },
  {
    name: "tap racing the native release",
    file: ".github/workflows/release-please.yml",
    before: "needs: [release-please, native]",
    after: "needs: [release-please]",
    finding: "only after the native release succeeds",
  },
  {
    name: "unsigned SHA256SUMS upload",
    file: ".github/workflows/release-native.yml",
    before: "dist/SHA256SUMS dist/SHA256SUMS.minisig dist/toolu.spdx.json",
    after: "dist/SHA256SUMS dist/toolu.spdx.json",
    finding: "must upload SHA256SUMS.minisig",
  },
  {
    name: "signing minisign differs from the installer pin",
    file: "install.sh",
    before:
      'MINISIGN_LINUX_SHA256="f0a0954413df8531befed169e447a66da6868d79052ed7e892e50a4291af7ae0"',
    after:
      'MINISIGN_LINUX_SHA256="0000000000000000000000000000000000000000000000000000000000000000"',
    finding: "the minisign archive install.sh pins",
  },
  {
    name: "inherited minisign secret",
    file: ".github/workflows/release-please.yml",
    before: "      TOOLU_MINISIGN_SECRET_KEY: ${{ secrets.TOOLU_MINISIGN_SECRET_KEY }}\n",
    after: "",
    finding: "TOOLU_MINISIGN_SECRET_KEY to the native release explicitly",
  },
  {
    name: "prerelease moving npm latest",
    file: ".github/workflows/npm-publish.yml",
    before: "npm_tag=next",
    after: "npm_tag=latest",
    finding: "latest dist-tag untouched",
  },
]) {
  test(`rejects ${scenario.name}`, () => {
    const root = fixture();
    mutate(root, scenario.file, scenario.before, scenario.after);
    expect(checkWorkflows(root).join("\n")).toContain(scenario.finding);
  });
}
