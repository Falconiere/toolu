/**
 * A GitHub release served on localhost for `install.sh`: signed `SHA256SUMS`,
 * real `toolu-<os>-<arch>.tar.gz` archives and a paged releases API. The
 * signer is the pinned minisign 0.11, downloaded once and checked by digest.
 */
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const MINISIGN = {
  linux: {
    file: "minisign-0.11-linux.tar.gz",
    sha256: "f0a0954413df8531befed169e447a66da6868d79052ed7e892e50a4291af7ae0",
  },
  darwin: {
    file: "minisign-0.11-macos.zip",
    sha256: "e7c410ae8b8960d7087392472b040bda9b2f307c76df0384ac37f9ad103fc893",
  },
} as const;

export const OS = process.platform === "darwin" ? "darwin" : "linux";
export const ARCH = process.arch === "arm64" ? "arm64" : "amd64";
export const ASSET = `toolu-${OS}-${ARCH}.tar.gz`;

export function sha256(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function run(argv: string[], cwd = process.cwd()): void {
  const result = Bun.spawnSync(argv, { cwd, stdout: "pipe", stderr: "pipe" });
  if (result.exitCode !== 0) {
    throw new Error(`${argv.join(" ")} failed: ${result.stderr.toString()}`);
  }
}

async function downloadMinisign(): Promise<string> {
  const pin = MINISIGN[OS];
  const cache = join(tmpdir(), `toolu-minisign-${pin.sha256.slice(0, 12)}-${ARCH}`);
  const binary = join(cache, "minisign");
  if (existsSync(binary)) return binary;
  const response = await fetch(
    `https://github.com/jedisct1/minisign/releases/download/0.11/${pin.file}`,
  );
  if (!response.ok) throw new Error(`minisign download: HTTP ${response.status}`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (sha256(bytes) !== pin.sha256)
    throw new Error("minisign download digest differs from the pin");
  const work = mkdtempSync(join(tmpdir(), "toolu-minisign-"));
  writeFileSync(join(work, pin.file), bytes);
  if (OS === "darwin") {
    run(["unzip", "-q", pin.file, "minisign"], work);
  } else {
    const cpu = ARCH === "arm64" ? "aarch64" : "x86_64";
    run(["tar", "-xzf", pin.file, `minisign-linux/${cpu}/minisign`], work);
    renameSync(join(work, `minisign-linux/${cpu}/minisign`), join(work, "minisign"));
  }
  mkdirSync(cache, { recursive: true });
  renameSync(join(work, "minisign"), binary);
  rmSync(work, { recursive: true, force: true });
  return binary;
}

let signer: Promise<{ minisign: string; pub: string; key: string }> | undefined;

/** The pinned minisign and a fixture keypair, made once per test process. */
export function fixtureSigner(): Promise<{ minisign: string; pub: string; key: string }> {
  signer ??= downloadMinisign().then((minisign) => {
    const dir = mkdtempSync(join(tmpdir(), "toolu-signer-"));
    const pub = join(dir, "fixture.pub");
    const key = join(dir, "fixture.key");
    run([minisign, "-G", "-W", "-p", pub, "-s", key]);
    return { minisign, pub, key };
  });
  return signer;
}

/** A `toolu` that prints `toolu <version>`, plus LICENSE, as release_native.py packs them. */
export function archive(version: string, members = ["toolu", "LICENSE"]): Uint8Array {
  const dir = mkdtempSync(join(tmpdir(), "toolu-archive-"));
  writeFileSync(join(dir, "toolu"), `#!/bin/sh\necho "toolu ${version}"\n`, { mode: 0o755 });
  writeFileSync(join(dir, "LICENSE"), "MIT\n");
  run(["tar", "-czf", "out.tar.gz", ...members], dir);
  const bytes = new Uint8Array(readFileSync(join(dir, "out.tar.gz")));
  rmSync(dir, { recursive: true, force: true });
  return bytes;
}

/** A signed `SHA256SUMS` over `files`, and its `.minisig`. */
export async function signedSums(
  files: Record<string, Uint8Array>,
): Promise<{ sums: Uint8Array; minisig: Uint8Array }> {
  const { minisign, key } = await fixtureSigner();
  const text = Object.entries(files)
    .map(([name, bytes]) => `${sha256(bytes)}  ${name}\n`)
    .join("");
  const dir = mkdtempSync(join(tmpdir(), "toolu-sums-"));
  writeFileSync(join(dir, "SHA256SUMS"), text);
  run([minisign, "-S", "-s", key, "-m", "SHA256SUMS", "-x", "SHA256SUMS.minisig"], dir);
  const read = (name: string) => new Uint8Array(readFileSync(join(dir, name)));
  const signed = { sums: read("SHA256SUMS"), minisig: read("SHA256SUMS.minisig") };
  rmSync(dir, { recursive: true, force: true });
  return signed;
}

/** A complete release: the platform archive, SHA256SUMS and its signature. */
export async function releaseFiles(version: string): Promise<Record<string, Uint8Array>> {
  const tarball = archive(version);
  const { sums, minisig } = await signedSums({ [ASSET]: tarball });
  return { [ASSET]: tarball, SHA256SUMS: sums, "SHA256SUMS.minisig": minisig };
}

export interface Release {
  tag: string;
  draft?: boolean;
  prerelease?: boolean;
  files: Record<string, Uint8Array>;
}

/** One release as the GitHub API shapes it, with the nesting the parser must skip. */
function releaseJson(release: Release, base: string): object {
  const user = { login: "toolu-bot", id: 1, type: "Bot", site_admin: false };
  return {
    url: `${base}/releases/${release.tag}`,
    id: 1,
    author: user,
    tag_name: release.tag,
    name: `toolu ${release.tag}`,
    draft: release.draft === true,
    prerelease: release.prerelease === true,
    assets: Object.keys(release.files).map((name, id) => ({ id, name, uploader: user, size: 1 })),
    body: 'Notes with "quotes", { "tag_name": "v99.0.0", "draft": true } and [brackets]\\',
  };
}

export interface ReleaseServer {
  url: string;
  requests: string[];
  stop(): void;
}

/** Serve `pages` of releases for `repo` under `/api` and their files under `/dl`. */
export function serveReleases(repo: string, pages: Release[][]): ReleaseServer {
  const requests: string[] = [];
  const server = Bun.serve({
    port: 0,
    hostname: "127.0.0.1",
    fetch(request) {
      const url = new URL(request.url);
      requests.push(url.pathname + url.search);
      if (url.pathname === `/api/repos/${repo}/releases`) {
        const page = Number(url.searchParams.get("page") ?? "1");
        const body = (pages[page - 1] ?? []).map((release) => releaseJson(release, url.origin));
        const headers = new Headers({ "content-type": "application/json" });
        if (page < pages.length) {
          const next = `${url.origin}/api/repos/${repo}/releases?per_page=100&page=${page + 1}`;
          headers.set("Link", `<${next}>; rel="next", <${next}>; rel="last"`);
        }
        return new Response(JSON.stringify(body), { headers });
      }
      const prefix = `/dl/${repo}/releases/download/`;
      if (url.pathname.startsWith(prefix)) {
        const [tag, name] = url.pathname.slice(prefix.length).split("/");
        const release = pages.flat().find((candidate) => candidate.tag === tag);
        const bytes = name === undefined ? undefined : release?.files[name];
        if (bytes !== undefined) return new Response(new Blob([new Uint8Array(bytes)]));
      }
      return new Response("not found", { status: 404 });
    },
  });
  return {
    url: `http://127.0.0.1:${server.port}`,
    requests,
    stop: () => {
      void server.stop(true);
    },
  };
}
