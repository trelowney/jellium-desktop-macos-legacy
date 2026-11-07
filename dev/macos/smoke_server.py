#!/usr/bin/env python3
"""Stand-in Jellyfin server for the macOS smoke test.

Usage: smoke_server.py <media URL> <port>

Answers /System/Info/Public like a real server, so the app accepts it, and
serves every other path a page that asks the app's player to play the media
URL. Nothing else of Jellyfin is needed to start the app and play.
"""
import http.server
import json
import sys

MEDIA_URL, PORT = sys.argv[1], int(sys.argv[2])

INFO = {
    "LocalAddress": f"http://127.0.0.1:{PORT}",
    "ServerName": "smoke-test",
    "Version": "10.11.0",
    "ProductName": "Jellyfin Server",
    "OperatingSystem": "",
    "Id": "00000000000000000000000000000000",
    "StartupWizardCompleted": True,
}

PAGE = """<!doctype html><meta charset="utf-8"><title>smoke test</title>
<script>
// Arguments: url, start ms, video, audio and subtitle track (-1 = none),
// metadata JSON, external audio and subtitle URLs, live stream.
function play() {
  if (!window.jmpNative || !window.jmpNative.playerLoad) return setTimeout(play, 500);
  console.info('smoke: playerLoad');
  window.jmpNative.playerLoad(%s, 0, 1, 1, -1, '{}', '', '', false);
}
addEventListener('load', play);
</script>""" % json.dumps(MEDIA_URL)


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path.split("?")[0].rstrip("/").lower().endswith("/system/info/public"):
            body, kind = json.dumps(INFO).encode(), "application/json"
        else:
            body, kind = PAGE.encode(), "text/html"
        self.send_response(200)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


http.server.ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
