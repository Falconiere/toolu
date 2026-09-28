/**
 * A loopback HTTP CONNECT proxy that tunnels every request to one local port.
 * Bun's fetch honours HTTPS_PROXY, so a client aimed at a real API hostname
 * lands on the fixture's TLS server without any test switch in its own code.
 */
import { connect, createServer, type Server, type Socket } from "node:net";

/** Called with each CONNECT authority (e.g. `api.exa.ai:443`); false drops the tunnel. */
export type OnConnect = (authority: string) => boolean;

const ESTABLISHED = "HTTP/1.1 200 Connection Established\r\n\r\n";

function tunnel(client: Socket, target: number, head: Buffer): void {
  const upstream = connect(target, "127.0.0.1", () => {
    client.write(ESTABLISHED);
    if (head.length > 0) upstream.write(head);
    client.pipe(upstream);
    upstream.pipe(client);
  });
  upstream.on("error", () => client.destroy());
  client.on("error", () => upstream.destroy());
}

function handle(client: Socket, target: number, onConnect: OnConnect): void {
  let buffered = Buffer.alloc(0);
  const onData = (chunk: Buffer): void => {
    buffered = Buffer.concat([buffered, chunk]);
    const end = buffered.indexOf("\r\n\r\n");
    if (end < 0) return;
    client.off("data", onData);
    const [verb = "", authority = ""] = buffered.subarray(0, end).toString().split(" ");
    if (verb !== "CONNECT" || !onConnect(authority)) {
      // A dropped tunnel is what a reset or unreachable host looks like to the client.
      client.destroy();
      return;
    }
    tunnel(client, target, buffered.subarray(end + 4));
  };
  client.on("data", onData);
  client.on("error", () => client.destroy());
}

/** Starts the proxy on an ephemeral loopback port. */
export function startConnectProxy(target: number, onConnect: OnConnect): Promise<Server> {
  const server = createServer((client) => handle(client, target, onConnect));
  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => resolve(server));
  });
}
