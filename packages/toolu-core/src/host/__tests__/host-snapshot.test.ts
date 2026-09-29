/**
 * Codex plugin snapshot against a real `codex` executable on PATH, byte-compared
 * with bash `toolu_snapshot_codex_plugins` run on the same CLI output.
 */
import { afterAll, describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import {
  codexPluginInstalled,
  codexPluginSnapshotPath,
  snapshotCodexPlugins,
} from "../host-snapshot.ts";

const HOST_SH = resolve(import.meta.dir, "../../../../../plugins/toolu/hooks/lib/host.sh");
const BASE_PATH = "/usr/bin:/bin";
const temps: string[] = [];
afterAll(() => {
  for (const dir of temps) rmSync(dir, { recursive: true, force: true });
});

function temp(): string {
  const dir = realpathSync(mkdtempSync(join(tmpdir(), "host-snapshot-")));
  temps.push(dir);
  return dir;
}

/** A bin dir whose `codex` prints `stdout` and exits `code`. */
function codexBin(stdout: string, code = 0): string {
  const bin = join(temp(), "bin");
  mkdirSync(bin);
  const body = `#!/bin/sh\ncat <<'JSON'\n${stdout}\nJSON\nexit ${code}\n`;
  writeFileSync(join(bin, "codex"), body);
  chmodSync(join(bin, "codex"), 0o755);
  return bin;
}

function codexEnv(path: string, snapshot: string): Record<string, string> {
  return {
    PATH: path,
    HOME: temp(),
    TOOLU_HOST_OVERRIDE: "codex",
    TOOLU_CODEX_PLUGIN_SNAPSHOT: snapshot,
  };
}

function bashSnapshot(env: Record<string, string>): string {
  const res = spawnSync("bash", ["-c", '. "$1"; toolu_snapshot_codex_plugins', "_", HOST_SH], {
    env: { ...env, PATH: `${env.PATH}:${process.env.PATH ?? BASE_PATH}` },
    encoding: "utf8",
  });
  expect(res.status).toBe(0);
  const path = env.TOOLU_CODEX_PLUGIN_SNAPSHOT ?? "";
  return readFileSync(path, "utf8");
}

const LISTED = JSON.stringify({
  installed: [
    { pluginId: "toolu@toolu", name: "toolu", marketplaceName: "toolu", installed: true },
    { pluginId: "statusline@toolu", installed: true, enabled: true },
    { name: "jev", marketplaceName: "toolu" },
    { pluginId: "off@toolu", installed: true, enabled: false },
    { pluginId: "gone@toolu", installed: false, enabled: true },
    { pluginId: "toolu@toolu", installed: true },
    { pluginId: "", name: "blank", marketplaceName: "toolu" },
    { pluginId: null, name: "fallback", marketplaceName: "toolu" },
    { pluginId: 7 },
  ],
  available: [],
});

describe("snapshotCodexPlugins", () => {
  test("canonicalises enabled installed plugins exactly as bash does", () => {
    const bin = codexBin(LISTED);
    const dir = temp();
    const tsPath = join(dir, "ts/plugins.json");
    const result = snapshotCodexPlugins({ env: codexEnv(`${bin}:${BASE_PATH}`, tsPath) });
    expect(result).toEqual({
      path: tsPath,
      written: true,
      snapshot: {
        version: 1,
        status: "ready",
        plugins: ["fallback@toolu", "jev@toolu", "statusline@toolu", "toolu@toolu"],
      },
    });
    const bashPath = join(dir, "bash/plugins.json");
    expect(readFileSync(tsPath, "utf8")).toBe(bashSnapshot(codexEnv(bin, bashPath)));
    expect(readdirSync(dirname(tsPath))).toEqual(["plugins.json"]);
  });

  for (const [name, stdout, code] of [
    ["non-JSON output", "not json", 0],
    ["installed is not an array", '{"installed":{}}', 0],
    ["a non-object entry", '{"installed":["toolu@toolu"]}', 0],
    ["a failing CLI", LISTED, 3],
  ] as const) {
    test(`writes indeterminate for ${name}, matching bash`, () => {
      const bin = codexBin(stdout, code);
      const dir = temp();
      const tsPath = join(dir, "ts.json");
      const result = snapshotCodexPlugins({ env: codexEnv(`${bin}:${BASE_PATH}`, tsPath) });
      expect(result?.snapshot).toEqual({ version: 1, status: "indeterminate", plugins: [] });
      expect(readFileSync(tsPath, "utf8")).toBe(
        bashSnapshot(codexEnv(bin, join(dir, "bash.json"))),
      );
    });
  }

  test("writes indeterminate when codex is not on PATH", () => {
    const path = join(temp(), "plugins.json");
    const result = snapshotCodexPlugins({ env: codexEnv(BASE_PATH, path) });
    expect(result?.snapshot.status).toBe("indeterminate");
    expect(readFileSync(path, "utf8")).toBe(
      '{"version":1,"status":"indeterminate","plugins":[]}\n',
    );
  });

  test("does nothing on a host other than Codex", () => {
    const path = join(temp(), "plugins.json");
    const env = {
      ...codexEnv(`${codexBin(LISTED)}:${BASE_PATH}`, path),
      TOOLU_HOST_OVERRIDE: "claude",
    };
    expect(snapshotCodexPlugins({ env })).toBeUndefined();
    expect(existsSync(path)).toBe(false);
  });

  test("an unwritable location reports written=false without throwing", () => {
    const blocker = join(temp(), "file");
    writeFileSync(blocker, "");
    const path = join(blocker, "sub/plugins.json");
    const result = snapshotCodexPlugins({ env: codexEnv(BASE_PATH, path) });
    expect(result?.written).toBe(false);
  });
});

describe("codexPluginSnapshotPath", () => {
  test("defaults under the Codex config root and honours the override, like bash", () => {
    const home = temp();
    const env = { PATH: BASE_PATH, HOME: home, TOOLU_HOST_OVERRIDE: "codex" };
    const bash = spawnSync(
      "bash",
      ["-c", '. "$1"; toolu_codex_plugin_snapshot_path', "_", HOST_SH],
      {
        env,
        encoding: "utf8",
      },
    );
    expect(codexPluginSnapshotPath({ env })).toBe(join(home, ".codex/toolu/codex-plugins.json"));
    expect(`${codexPluginSnapshotPath({ env })}\n`).toBe(bash.stdout);
    const custom = { ...env, TOOLU_CODEX_PLUGIN_SNAPSHOT: "/x/snap.json" };
    expect(codexPluginSnapshotPath({ env: custom })).toBe("/x/snap.json");
  });
});

describe("codexPluginInstalled", () => {
  function lookup(body: string | null, spec: string) {
    const path = join(temp(), "snap.json");
    if (body !== null) writeFileSync(path, body);
    return codexPluginInstalled(spec, { env: { TOOLU_CODEX_PLUGIN_SNAPSHOT: path, HOME: "/h" } });
  }
  const ready = '{"version":1,"status":"ready","plugins":["toolu@toolu"]}';

  test("a ready snapshot answers installed or absent", () => {
    expect(lookup(ready, "toolu@toolu")).toBe("installed");
    expect(lookup(ready, "fixture@toolu")).toBe("absent");
    expect(
      lookup('{"version":1,"status":"ready","plugins":["toolu@another"]}', "toolu@toolu"),
    ).toBe("absent");
  });

  test("an empty spec is absent without reading the snapshot", () => {
    expect(lookup(null, "")).toBe("absent");
  });

  test("anything short of a valid ready snapshot is unknown", () => {
    expect(lookup(null, "toolu@toolu")).toBe("unknown");
    expect(lookup("not json", "toolu@toolu")).toBe("unknown");
    expect(lookup('{"version":2,"status":"ready","plugins":[]}', "toolu@toolu")).toBe("unknown");
    expect(lookup('{"version":1,"status":"indeterminate","plugins":[]}', "toolu@toolu")).toBe(
      "unknown",
    );
    expect(lookup('{"version":1,"status":"ready","plugins":{}}', "toolu@toolu")).toBe("unknown");
  });
});
