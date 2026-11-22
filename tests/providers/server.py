"""Synthetic provider responses. Binds exclusively to an ephemeral loopback port."""
from datetime import datetime, timezone, timedelta
from email.utils import format_datetime
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import gzip
import json
from vector_tile import harbour
import sys
import time
from urllib.parse import urlsplit, parse_qs

work = Path(sys.argv[1])
modified = "Sun, 09 Mar 2025 06:00:00 GMT"

def forecast():
    start = datetime(2025, 3, 9, 7, tzinfo=timezone.utc)
    hours = []
    for i, symbol in enumerate(["partlycloudy", "fair", "clearsky", "lightrain", "rain", "cloudy"]):
        hours.append({"time": (start + timedelta(hours=i)).isoformat(), "data": {
            "instant": {"details": {"air_temperature": 4.5+i, "wind_speed": 3.2}},
            "next_1_hours": {"summary": {"symbol_code": symbol+"_day"}}}})
    return {"properties":{"meta":{"updated_at":"2025-03-09T06:00:00Z",
            "units":{"air_temperature":"celsius","wind_speed":"m/s"}}, "timeseries":hours}}

class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_GET(self):
        url = urlsplit(self.path)
        with (work / "requests").open("a") as f:
            print(json.dumps([self.path, self.headers["User-Agent"], self.headers["If-Modified-Since"], time.monotonic()]), file=f)
        if url.path == "/forecast":
            if (work / "unavailable").exists():
                self.send_error(503)
                return
            checked = self.headers["If-Modified-Since"] == modified
            expires = datetime.now(timezone.utc) + timedelta(hours=1 if checked or (work/"fresh").exists() else -1)
            self.send_response(304 if checked else 200)
            self.send_header("Expires", format_datetime(expires, usegmt=True))
            self.send_header("Last-Modified", modified)
            self.end_headers()
            if not checked:
                self.wfile.write(json.dumps(forecast()).encode())
        elif url.path == "/tilejson":
            self.send_response(200)
            self.end_headers()
            self.wfile.write(json.dumps({"tilejson":"3.0.0", "minzoom":0, "maxzoom":14,
                             "tiles":["/tiles/{z}/{x}/{y}.pbf"]}).encode())
        elif url.path.startswith("/tiles/") and not (work / "missing").exists():
            z, x, y = map(int, url.path.removesuffix(".pbf").split("/")[2:])
            self.send_response(200)
            self.send_header("Content-Encoding", "gzip")
            self.end_headers()
            self.wfile.write(gzip.compress(harbour(z,x,y)))
        elif url.path == "/search":
            assert parse_qs(url.query)["format"] == ["jsonv2"]
            self.send_response(200)
            self.end_headers()
            places = [
                {"display_name":"Tide Pool, Survey Coast", "lat":"59.926", "lon":"10.72"},
                {"display_name":"Harbour Point, Survey Coast", "lat":"59.9139", "lon":"10.7522"}
            ]
            if (work / "single-result").exists():
                places = places[:1]
            self.wfile.write(json.dumps(places).encode())
        else:
            self.send_error(404)

server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
(work / "address").write_text(f"http://127.0.0.1:{server.server_port}")
server.serve_forever()
