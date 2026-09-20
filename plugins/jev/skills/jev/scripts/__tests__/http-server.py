"""Loopback HTTPS fixture: exercise the real CLI, curl, retries, and JSON IO."""

import json
import ssl
import sys
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

root = Path(sys.argv[1])


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        record = {
            "path": self.path,
            "authorization": self.headers.get("Authorization"),
            "content_type": self.headers.get("Content-Type"),
            "body": body,
        }
        with (root / "requests.jsonl").open("a") as log:
            log.write(json.dumps(record) + "\n")
        plan_file = root / "responses.json"
        if plan_file.exists():
            plan = json.loads(plan_file.read_text())
            # Repeat the final response until teardown (including persistent errors).
            response = plan.pop(0) if len(plan) > 1 else plan[0]
            pending = plan_file.with_suffix(".tmp")
            pending.write_text(json.dumps(plan))
            pending.replace(plan_file)
        else:
            response = {"status": 200}
        time.sleep(response.get("delay", 0))
        self.send_response(response["status"])
        self.send_header("Content-Type", "application/json")
        if "retry_after" in response:
            self.send_header("Retry-After", str(response["retry_after"]))
        if "retry_after_ms" in response:
            self.send_header("retry-after-ms", str(response["retry_after_ms"]))
        self.end_headers()
        response_file = root / "response.json"
        if "body" in response:
            payload = response["body"]
        elif response_file.exists():
            payload = response_file.read_text()
        else:
            answers = {}
            for qid, question in body["questions"].items():
                kind = question["type"]
                answer = {"type": kind}
                if kind == "noul":
                    answer["noul"] = 0.92
                elif kind == "choice":
                    options = list(question["criteria"])
                    answer.update(choice=options[0], confidence=1,
                                  probabilities={k: int(i == 0) for i, k in enumerate(options)})
                else:
                    levels = question["criteria"]
                    answer.update(score=0, confidence=1,
                                  legend={str(i): v for i, v in enumerate(levels)},
                                  probabilities={str(i): int(i == 0) for i in range(len(levels))})
                answers[qid] = answer
            payload = json.dumps({"model": "jev-1.13.0", "answers": answers,
                                  "usage": {"input_tokens": 312, "output_tokens": 48}})
        try:
            self.wfile.write(payload.encode())
        except (BrokenPipeError, ConnectionResetError, ssl.SSLEOFError):
            pass


server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(root / "cert.pem", root / "key.pem")
server.socket = context.wrap_socket(server.socket, server_side=True)
(root / "port").write_text(str(server.server_port))
server.serve_forever()
