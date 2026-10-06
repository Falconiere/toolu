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
  return root;
}

function mutate(root: string, file: string, before: string, after: string): void {
  const path = join(root, file);
  const text = readFileSync(path, "utf8");
  expect(text).toContain(before);
  writeFileSync(path, text.replace(before, after));
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
