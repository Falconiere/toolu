/** Run the real `install.sh` against a per-test localhost release. */
import { chmodSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import {
  type Release,
  type ReleaseServer,
  fixtureSigner,
  releaseFiles,
  serveReleases,
} from "./install-fixture.ts";

const INSTALL_SH = resolve(import.meta.dir, "../../../install.sh");
export const REPO = "Falconiere/toolu";

export interface Sandbox {
  scratch: () => string;
  serve: (pages: Release[][], repo?: string) => ReleaseServer;
}

/** Each test owns its directories and servers; they go when it ends. */
export function sandboxed(body: (box: Sandbox) => Promise<void>): () => Promise<void> {
  return async () => {
    const cleanups: Array<() => void> = [];
    const box: Sandbox = {
      scratch: () => {
        const dir = mkdtempSync(join(tmpdir(), "toolu-install-"));
        cleanups.push(() => {
          chmodSync(dir, 0o755);
          rmSync(dir, { recursive: true, force: true });
        });
        return dir;
      },
      serve: (pages, repo = REPO) => {
        const server = serveReleases(repo, pages);
        cleanups.push(() => server.stop());
        return server;
      },
    };
    try {
      await body(box);
    } finally {
      for (const cleanup of cleanups) cleanup();
    }
  };
}

/** A complete stable release whose `toolu` prints `toolu <version>`. */
export async function stable(version: string): Promise<Release> {
  return { tag: `v${version}`, files: await releaseFiles(version) };
}

export interface Run {
  exitCode: number;
  stdout: string;
  stderr: string;
}

/** `bash install.sh <args>` with the fixture verifier, the server standing in for GitHub. */
export async function install(
  server: ReleaseServer | undefined,
  args: string[],
  env: Record<string, string> = {},
): Promise<Run> {
  const { minisign, pub } = await fixtureSigner();
  const base = server?.url ?? "http://127.0.0.1:9";
  const proc = Bun.spawn(["bash", INSTALL_SH, ...args], {
    env: {
      PATH: process.env.PATH ?? "/usr/bin:/bin",
      HOME: tmpdir(),
      TMPDIR: tmpdir(),
      TOOLU_GITHUB_API: `${base}/api`,
      TOOLU_DOWNLOAD_BASE: `${base}/dl`,
      TOOLU_MINISIGN: minisign,
      TOOLU_MINISIGN_PUB: pub,
      ...env,
    },
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  return { exitCode, stdout, stderr };
}

/** What the installed `toolu` in `dir` prints. */
export function runInstalled(dir: string): string {
  return Bun.spawnSync([join(dir, "toolu")])
    .stdout.toString()
    .trim();
}

/** A PATH whose `uname` is `script`, ahead of the real tools. */
export function fakeUname(dir: string, script: string): string {
  writeFileSync(join(dir, "uname"), `#!/bin/sh\n${script}\n`, { mode: 0o755 });
  return `${dir}:${process.env.PATH ?? "/usr/bin:/bin"}`;
}
