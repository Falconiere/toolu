# Control request deadline: release resource evidence

Measured 2026-10-29 on macOS Darwin 27.0.0, arm64 (macmini). Both binaries were built with `cargo build --release -p toolu-cli`. Baseline source is `origin/main` at `5abc923924363082be7ced8ce8fbd6ff7f2614ac`; fixed source is this branch with the request deadline diff. The measurement uses a fresh `TOOLU_RESOURCE_HOME` and config directory for each sample.

Each sample opens 24 real Unix control peers (12 send nothing, 12 send `{"op":` without a newline), reads the greeting, and leaves them connected. It samples process threads with `ps -M -p <pid>` and numeric descriptors with `lsof -p <pid> -Ff` before connection, just after connection, and 6.3 seconds later. It then opens 24 more stalled peers, sends a valid `stop` request, and times process exit. Three independent samples were run per build.

| Build | Samples | Threads idle / loaded / after 6.3 s | FDs idle / loaded / after 6.3 s | Peers closed by 6.3 s | First close | Stop with 24 new stalled peers |
| --- | ---: | --- | --- | ---: | --- | --- |
| Baseline | 3 | 5 / 29 / 29 in every sample | 7 / 31 / 31 in every sample | 0 / 24 in every sample | None | 0.034–0.045 s; process exit closes sessions |
| Fixed | 3 | 5 / 29 / 5 in every sample | 7 / 31 / 7 in every sample | 24 / 24 in every sample | 4.978–4.988 s | 4.915–5.196 s; server joins sessions |

The baseline retains all 24 session threads and descriptors while the process remains alive. The fixed engine releases them after the five-second request bound. Shutdown intentionally waits for incomplete sessions to finish within that bound; observed process exit includes scheduler and process overhead. This is a resource-retention measurement, not a throughput or CPU speedup claim. The real-socket Rust regressions also cover byte trickle, accepted late `wait`/`replace` replies, malformed and oversized requests, protocol checks, permissions, and shutdown.

## Reproduce

From a clean checkout with the branch diff applied:

```sh
mkdir -p /tmp/toolu-748-base
TOOLU_748_BASE=5abc923924363082be7ced8ce8fbd6ff7f2614ac
git archive "$TOOLU_748_BASE" | tar -x -C /tmp/toolu-748-base
(cd /tmp/toolu-748-base && cargo build --release -p toolu-cli)
cargo build --release -p toolu-cli
# Save the Python block below as /tmp/toolu-748-measure.py.
toolu epic job -- python3 /tmp/toolu-748-measure.py /tmp/toolu-748-base/target/release/toolu baseline
toolu epic job -- python3 /tmp/toolu-748-measure.py target/release/toolu fixed
```

```python
import json
import os
import pathlib
import re
import select
import socket
import subprocess
import sys
import tempfile
import time

binary = pathlib.Path(sys.argv[1]).resolve()
label = sys.argv[2]

def threads(pid):
    rows = subprocess.check_output(['ps', '-M', '-p', str(pid)], text=True).splitlines()
    return len(rows) - 1

def fds(pid):
    rows = subprocess.check_output(['lsof', '-p', str(pid), '-Ff'], text=True).splitlines()
    return sum(bool(re.fullmatch(r'f\d+', row)) for row in rows)

def read_line(peer):
    data = b''
    while not data.endswith(b'\n'):
        data += peer.recv(4096)
    return json.loads(data)

for sample in range(1, 4):
    with tempfile.TemporaryDirectory(prefix='toolu-748-measure-') as root:
        resource = pathlib.Path(root, 'resources')
        resource.mkdir()
        config = pathlib.Path(root, 'config')
        config.mkdir()
        log = open(pathlib.Path(root, 'serve.log'), 'w+')
        env = os.environ.copy()
        env['TOOLU_RESOURCE_HOME'] = str(resource)
        process = subprocess.Popen([str(binary), 'epic', 'serve', '--no-http', '--host', 'codex', '--config-dir', str(config)], env=env, stdout=log, stderr=subprocess.STDOUT)
        peers = []
        try:
            path = resource / 'engine.sock'
            for _ in range(100):
                if path.exists():
                    break
                if process.poll() is not None:
                    raise RuntimeError('engine exited before bind')
                time.sleep(0.05)
            if not path.exists():
                raise RuntimeError('engine did not bind')
            def connect():
                peer = socket.socket(socket.AF_UNIX)
                peer.settimeout(8)
                peer.connect(str(path))
                hello = read_line(peer)
                assert hello['op'] == 'hello', hello
                return peer, hello['protocol']
            first, protocol = connect()
            first.close()
            idle_threads, idle_fds = threads(process.pid), fds(process.pid)
            for i in range(24):
                peer, _ = connect()
                if i % 2:
                    peer.sendall(b'{"op":')
                peers.append(peer)
            time.sleep(0.2)
            loaded_threads, loaded_fds = threads(process.pid), fds(process.pid)
            started = time.monotonic()
            closed = set()
            first_closed = None
            while time.monotonic() - started < 6.3 and len(closed) < len(peers):
                ready, _, _ = select.select([peer for i, peer in enumerate(peers) if i not in closed], [], [], 0.1)
                for peer in ready:
                    if peer.recv(1) == b'':
                        closed.add(peers.index(peer))
                        if first_closed is None:
                            first_closed = round(time.monotonic() - started + 0.2, 3)
            if len(closed) == len(peers):
                remaining = 6.3 - (time.monotonic() - started)
                if remaining > 0:
                    time.sleep(remaining)
            aged_threads, aged_fds = threads(process.pid), fds(process.pid)
            for i in range(24):
                peer, _ = connect()
                if i % 2:
                    peer.sendall(b'{"op":')
                peers.append(peer)
            time.sleep(0.1)
            stop, _ = connect()
            stop.sendall((json.dumps({'op': 'stop', 'protocol': protocol}) + '\n').encode())
            stop_reply = read_line(stop)
            stop.close()
            stop_started = time.monotonic()
            process.wait(timeout=8)
            stop_seconds = round(time.monotonic() - stop_started, 3)
            print(json.dumps({'label': label, 'sample': sample, 'idle_threads': idle_threads, 'loaded_threads': loaded_threads, 'aged_threads': aged_threads, 'idle_fds': idle_fds, 'loaded_fds': loaded_fds, 'aged_fds': aged_fds, 'closed_by_6s': len(closed), 'first_closed_seconds': first_closed, 'stop_seconds': stop_seconds, 'stop_reply': stop_reply, 'exit': process.returncode}), flush=True)
        except Exception:
            log.seek(0)
            print(log.read(), file=sys.stderr)
            raise
        finally:
            for peer in peers:
                peer.close()
            if process.poll() is None:
                process.kill()
                process.wait()
            log.close()
```
