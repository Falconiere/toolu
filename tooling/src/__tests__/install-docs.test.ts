import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dir, "../../..");
const read = (path: string): string => readFileSync(resolve(root, path), "utf8");

/** The first ```bash fence after `heading`. */
function firstBashFence(text: string, heading: string): string {
  const section = text.slice(text.indexOf(`\n${heading}\n`));
  const match = /```bash\n([\s\S]*?)```/.exec(section);
  return match?.[1] ?? "";
}

test("the README Install section leads with the two install commands and the plugins (AC-11)", () => {
  const fence = firstBashFence(read("README.md"), "## Install");
  const commands = fence.split("\n").map((line) => line.replace(/\s+#.*$/, "").trim());
  expect(commands.slice(0, 3)).toEqual([
    "curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash",
    "brew install falconiere/tap/toolu",
    "toolu plugins install",
  ]);
});

test("docs/install.md covers upgrade, uninstall, pinning, rollback and forks (AC-11)", () => {
  const text = read("docs/install.md");
  for (const needle of [
    "brew upgrade toolu",
    "brew uninstall toolu",
    "bash -s -- --uninstall",
    "bash -s -- --version v1.2.3",
    "**Rollback.**",
    "--install-dir ~/.local/bin",
    "TOOLU_REPO=example/toolu bash",
    "--check",
  ]) {
    expect(text).toContain(needle);
  }
});

test("the release notes name the signature and the stable-only tap", () => {
  const text = read("docs/releases/native.md");
  expect(text).toContain("SHA256SUMS.minisig");
  expect(text).toContain("release-homebrew.yml");
  expect(text).toContain("a prerelease leaves the tap unchanged");
});
