"""A tiny local web shop used by the guide's example plan. Run it with: python demo-server.py [port]

  GET  /            an HTML page with a hidden form field (csrf)
  POST /login       JSON {"token": "..."} (a new value on every call)
  GET  /cart        200 when ?token= is a token this server issued, 401 otherwise
  GET  /slow        answers after about 300 ms
  GET  /fail        500
"""
import json, random, sys, time, uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse, parse_qs

issued = set()


class Shop(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def reply(self, status, body, kind="text/plain"):
        data = body.encode()
        self.send_response(status)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        url = urlparse(self.path)
        time.sleep(random.uniform(0.005, 0.04))
        if url.path == "/":
            self.reply(200, '<html><form><input type="hidden" name="csrf" value="%s"></form></html>' % uuid.uuid4().hex[:8], "text/html")
        elif url.path == "/cart":
            ok = parse_qs(url.query).get("token", [""])[0] in issued
            self.reply(200 if ok else 401, "cart" if ok else "unauthorised")
        elif url.path == "/slow":
            time.sleep(0.3)
            self.reply(200, "slow")
        elif url.path == "/fail":
            self.reply(500, "boom")
        else:
            self.reply(404, "not found")

    def do_POST(self):
        n = int(self.headers.get("Content-Length") or 0)
        self.rfile.read(n)
        if self.path.startswith("/login"):
            token = uuid.uuid4().hex
            issued.add(token)
            self.reply(200, json.dumps({"token": token}), "application/json")
        else:
            self.reply(404, "not found")


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8089
    ThreadingHTTPServer(("127.0.0.1", port), Shop).serve_forever()
