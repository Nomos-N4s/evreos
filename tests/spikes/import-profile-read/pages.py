"""Serve pages that navigate onward on their own, so a browser left on them
writes a new history visit every INTERVAL milliseconds."""
import http.server, sys

INTERVAL = int(sys.argv[2]) if len(sys.argv) > 2 else 150

class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        try:
            n = int(self.path.rsplit('/', 1)[-1])
        except ValueError:
            n = 1
        body = (f"<!doctype html><title>Page {n} Σελίδα</title>"
                f"<p>page {n}</p><script>setTimeout(()=>location.href='/p/{n+1}',{INTERVAL})</script>")
        data = body.encode()
        self.send_response(200)
        self.send_header('Content-Type', 'text/html; charset=utf-8')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)
    def log_message(self, *a):
        pass

http.server.ThreadingHTTPServer(('127.0.0.1', int(sys.argv[1])), H).serve_forever()
