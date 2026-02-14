"""SPA server with reverse proxy for API routes.

Serves static files from frontend/dist with SPA fallback (index.html),
and proxies /api/* requests to the backend on port 5150.
"""
import http.server
import pathlib
import urllib.request
import urllib.error
import sys

BACKEND = "http://localhost:5150"
DIST_DIR = sys.argv[1] if len(sys.argv) > 1 else "dist"
PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 5173


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=DIST_DIR, **kwargs)

    def _proxy(self):
        length = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(length) if length else None
        url = BACKEND + self.path
        headers = {
            k: v for k, v in self.headers.items() if k.lower() != "host"
        }
        req = urllib.request.Request(
            url, data=body, method=self.command, headers=headers
        )
        try:
            resp = urllib.request.urlopen(req)
            code = resp.status
        except urllib.error.HTTPError as e:
            resp = e
            code = e.code
        self.send_response(code)
        for k, v in resp.headers.items():
            if k.lower() not in ("transfer-encoding", "connection"):
                self.send_header(k, v)
        self.end_headers()
        self.wfile.write(resp.read())

    def do_GET(self):
        if self.path.startswith("/api/"):
            return self._proxy()
        if not pathlib.Path(self.translate_path(self.path)).exists():
            self.path = "/index.html"
        super().do_GET()

    def do_POST(self):
        if self.path.startswith("/api/"):
            return self._proxy()
        self.send_error(405)

    def do_PUT(self):
        if self.path.startswith("/api/"):
            return self._proxy()
        self.send_error(405)

    def do_DELETE(self):
        if self.path.startswith("/api/"):
            return self._proxy()
        self.send_error(405)

    def log_message(self, fmt, *args):
        # Prefix logs for easier debugging in CI
        sys.stderr.write(f"[SPA-Proxy] {fmt % args}\n")


if __name__ == "__main__":
    print(f"Serving {DIST_DIR} on :{PORT}, proxying /api/* -> {BACKEND}")
    http.server.HTTPServer(("", PORT), Handler).serve_forever()
