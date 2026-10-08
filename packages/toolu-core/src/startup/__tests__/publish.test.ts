/** Stable-path publishing against real config roots, links and files (#269). */
import { spawnSync } from "node:child_process";
import { afterAll, expect, test } from "bun:test";
import {
  chmodSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  readlinkSync,
  realpathSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { bunAdvisory, bunOnPath, publishWrapper } from "../publish.ts";

const temps: string[] = [];
afterAll(() => {
  for (const dir of temps) rmSync(dir, { recursive: true, force: true });
});

function temp(): string {
  const dir = realpathSync(mkdtempSync(join(tmpdir(), "startup-publish-")));
  temps.push(dir);
  return dir;
}

/** A temp tree with a real source file to publish. */
function fixture(): { root: string; source: string } {
  const root = temp();
  const source = join(root, "plugin/hooks/dist/jev.js");
  mkdirSync(dirname(source), { recursive: true });
  writeFileSync(source, "#!/usr/bin/env bun\n");
  return { root, source };
}

const base = { plugin: "jev", dir: "jev", name: "jev.sh" };

test.concurrent("publishes a symlink under CLAUDE_CONFIG_DIR on the Claude host", () => {
  const { root, source } = fixture();
  const env = { HOME: root, CLAUDE_CONFIG_DIR: join(root, "cfg") };
  const result = publishWrapper({ ...base, source, env });
  const dst = join(root, "cfg/jev/jev.sh");
  expect(result).toEqual({ status: "published", path: dst });
  expect(readlinkSync(dst)).toBe(source);
});

test.concurrent("publishes under CODEX_HOME when PLUGIN_ROOT marks the Codex host", () => {
  const { root, source } = fixture();
  const env = { HOME: root, PLUGIN_ROOT: "/p", CODEX_HOME: join(root, "codex home") };
  publishWrapper({ ...base, source, env });
  expect(readlinkSync(join(root, "codex home/jev/jev.sh"))).toBe(source);
  expect(existsSync(join(root, ".claude"))).toBe(false);
});

test.concurrent("TOOLU_CONFIG_DIR beats both native roots", () => {
  const { root, source } = fixture();
  const env = {
    HOME: root,
    TOOLU_HOST_OVERRIDE: "codex",
    CODEX_HOME: join(root, "codex"),
    TOOLU_CONFIG_DIR: join(root, "custom"),
  };
  publishWrapper({ ...base, source, env });
  expect(readlinkSync(join(root, "custom/jev/jev.sh"))).toBe(source);
  expect(existsSync(join(root, "codex"))).toBe(false);
});

test.concurrent("replaces a stale or broken symlink with the current target", () => {
  const { root, source } = fixture();
  const env = { HOME: root, TOOLU_CONFIG_DIR: join(root, "cfg") };
  mkdirSync(join(root, "cfg/jev"), { recursive: true });
  symlinkSync("/nonexistent/old/jev.sh", join(root, "cfg/jev/jev.sh"));
  expect(publishWrapper({ ...base, source, env }).status).toBe("published");
  expect(readlinkSync(join(root, "cfg/jev/jev.sh"))).toBe(source);
});

test.concurrent("never touches a regular file or directory at the published path", () => {
  const { root, source } = fixture();
  const env = { HOME: root, TOOLU_CONFIG_DIR: join(root, "cfg") };
  const file = join(root, "cfg/jev/jev.sh");
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, "#!/usr/bin/env bash\necho user-override\n");
  expect(publishWrapper({ ...base, source, env })).toEqual({
    status: "kept-user-file",
    path: file,
  });
  expect(lstatSync(file).isSymbolicLink()).toBe(false);
  expect(readFileSync(file, "utf8")).toBe("#!/usr/bin/env bash\necho user-override\n");

  const dirAtPath = join(root, "cfg/jev/other.sh");
  mkdirSync(dirAtPath);
  const other = publishWrapper({ ...base, name: "other.sh", source, env });
  expect(other.status).toBe("kept-user-file");
  expect(lstatSync(dirAtPath).isDirectory()).toBe(true);
});

test.concurrent("a second run leaves the same link and no temp litter", () => {
  const { root, source } = fixture();
  const env = { HOME: root, TOOLU_CONFIG_DIR: join(root, "cfg") };
  publishWrapper({ ...base, source, env });
  const before = lstatSync(join(root, "cfg/jev/jev.sh")).ino;
  expect(publishWrapper({ ...base, source, env }).status).toBe("published");
  expect(lstatSync(join(root, "cfg/jev/jev.sh")).ino).toBe(before);
  expect(readlinkSync(join(root, "cfg/jev/jev.sh"))).toBe(source);
});

test.concurrent("a missing source publishes nothing and creates no directory", () => {
  const root = temp();
  const env = { HOME: root, TOOLU_CONFIG_DIR: join(root, "cfg") };
  const lines: string[] = [];
  const result = publishWrapper({
    ...base,
    source: join(root, "absent.js"),
    env,
    warn: (line) => lines.push(line),
  });
  expect(result).toEqual({ status: "source-missing" });
  expect(existsSync(join(root, "cfg"))).toBe(false);
  expect(lines).toEqual([]);
});

test.concurrent("an uncreatable config dir warns once and publishes nothing", () => {
  const { root, source } = fixture();
  writeFileSync(join(root, "blocker"), "");
  const env = { HOME: root, TOOLU_CONFIG_DIR: join(root, "blocker") };
  const lines: string[] = [];
  const result = publishWrapper({
    ...base,
    plugin: "toolu-review",
    what: "helper",
    source,
    env,
    warn: (line) => lines.push(line),
  });
  const dir = join(root, "blocker/jev");
  expect(result).toEqual({ status: "unwritable", path: dir });
  expect(lines).toEqual([`toolu-review: cannot create ${dir} — helper not published`]);
});

test.concurrent("bunOnPath reads the PATH it is given", () => {
  expect(bunOnPath({ PATH: dirname(process.execPath) })).toBe(true);
  expect(bunOnPath({ PATH: temp() })).toBe(false);
  expect(bunOnPath({})).toBe(false);
});

test.concurrent("bunAdvisory is the bash hooks' exact line", () => {
  expect(bunAdvisory("jev", "jev search CLI")).toBe(
    "jev: bun not found on PATH — the jev search CLI needs Bun 1.4.x (https://bun.sh; see docs/runtime.md)",
  );
});

test.concurrent("a directory that refuses the link reports link-failed and leaves no litter", () => {
  const { root, source } = fixture();
  const env = { HOME: root, TOOLU_CONFIG_DIR: join(root, "cfg") };
  const dir = join(root, "cfg/jev");
  mkdirSync(dir, { recursive: true });
  // Root creates a symlink in a mode-0555 directory. The immutable flag stops it.
  const rootUser = process.getuid?.() === 0;
  if (rootUser) {
    const marked = spawnSync("chattr", ["+i", dir]);
    expect(marked.status).toBe(0);
  } else chmodSync(dir, 0o555);
  try {
    const result = publishWrapper({ ...base, source, env });
    expect(result).toEqual({ status: "link-failed", path: join(dir, "jev.sh") });
    expect(readdirSync(dir)).toEqual([]);
  } finally {
    if (rootUser) spawnSync("chattr", ["-i", dir]);
    else chmodSync(dir, 0o755);
  }
});
