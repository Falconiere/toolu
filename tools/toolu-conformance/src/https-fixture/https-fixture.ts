/**
 * Private loopback HTTPS fixture for REST skill CLIs (#270). The real CLI runs
 * as a subprocess with `env` applied: its fetch goes through a CONNECT proxy to
 * a local TLS server whose throwaway self-signed certificate names the real API
 * hosts and is trusted through NODE_EXTRA_CA_CERTS. TLS verification stays on.
 * Every request is recorded; responses follow a plan whose last entry repeats.
 */
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import type { Server } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { startConnectProxy } from "./connect-proxy.ts";

export interface PlannedResponse {
  readonly status?: number;
  readonly body?: string;
  readonly contentType?: string;
  /** Drop the tunnel instead of answering: the client sees a transport failure. */
  readonly close?: boolean;
}

export interface RecordedRequest {
  readonly method: string;
  /** Path plus query string, exactly as sent. */
  readonly path: string;
  /** Lower-cased header names. */
  readonly headers: Readonly<Record<string, string>>;
  readonly body: string;
}

export interface HttpsFixture {
  /** Environment for the CLI subprocess: proxy, CA bundle, no proxy bypass. */
  readonly env: Readonly<Record<string, string>>;
  /** CONNECT authorities in arrival order, e.g. `api.exa.ai:443`. */
  readonly connects: readonly string[];
  readonly requests: readonly RecordedRequest[];
  /** Replaces the response plan and clears the recordings. */
  plan(responses: readonly PlannedResponse[]): void;
  stop(): Promise<void>;
}

const DEFAULT_RESPONSE: PlannedResponse = { status: 200, body: "{}" };

function selfSigned(dir: string, hosts: readonly string[]): { cert: string; key: string } {
  const cert = join(dir, "cert.pem");
  const key = join(dir, "key.pem");
  const san = hosts.map((host) => `DNS:${host}`).join(",");
  const args = ["req", "-x509", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:prime256v1"];
  args.push("-nodes", "-keyout", key, "-out", cert, "-days", "1");
  args.push("-subj", `/CN=${hosts[0] ?? "localhost"}`, "-addext", `subjectAltName=${san}`);
  const run = spawnSync("openssl", args, { encoding: "utf8" });
  if (run.status !== 0) throw new Error(`openssl failed: ${run.stderr || String(run.error)}`);
  return { cert, key };
}

function closeProxy(proxy: Server): Promise<void> {
  return new Promise((resolve) => proxy.close(() => resolve()));
}

/** Starts the fixture for the given API hostnames. */
export async function startHttpsFixture(hosts: readonly string[]): Promise<HttpsFixture> {
  const dir = mkdtempSync(join(tmpdir(), "toolu-https-fixture-"));
  const { cert, key } = selfSigned(dir, hosts);
  let queue: PlannedResponse[] = [];
  const connects: string[] = [];
  const requests: RecordedRequest[] = [];
  const next = (): PlannedResponse =>
    (queue.length > 1 ? queue.shift() : queue[0]) ?? DEFAULT_RESPONSE;

  const tls = Bun.serve({
    hostname: "127.0.0.1",
    port: 0,
    tls: { cert: readFileSync(cert, "utf8"), key: readFileSync(key, "utf8") },
    async fetch(request) {
      const url = new URL(request.url);
      const body = await request.text();
      requests.push({
        method: request.method,
        path: url.pathname + url.search,
        headers: request.headers.toJSON(),
        body,
      });
      const response = next();
      return new Response(response.body ?? "{}", {
        status: response.status ?? 200,
        headers: { "content-type": response.contentType ?? "application/json" },
      });
    },
  });
  const proxy = await startConnectProxy(tls.port ?? 0, (authority) => {
    connects.push(authority);
    if (queue[0]?.close !== true) return true;
    next();
    return false;
  });
  const address = proxy.address();
  const proxyUrl = `http://127.0.0.1:${typeof address === "object" && address ? address.port : 0}`;

  return {
    env: {
      HTTPS_PROXY: proxyUrl,
      https_proxy: proxyUrl,
      NO_PROXY: "",
      no_proxy: "",
      NODE_EXTRA_CA_CERTS: cert,
    },
    connects,
    requests,
    plan(responses) {
      queue = [...responses];
      connects.length = 0;
      requests.length = 0;
    },
    async stop() {
      await closeProxy(proxy);
      await tls.stop(true);
      rmSync(dir, { recursive: true, force: true });
    },
  };
}
