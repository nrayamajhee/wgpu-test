#!/usr/bin/env python3

from http.cookies import SimpleCookie
from http.server import HTTPServer, SimpleHTTPRequestHandler, test
import sys

session = SimpleCookie()
session["state"] = "latest"

class RequestHandler(SimpleHTTPRequestHandler):
    def respond(self, message):
        self.send_response(200)
        self.end_headers()
        self.wfile.write(bytes(message, "utf8"))

    def send_error(self, code, message):
        if self.path == "/check-state":
            self.respond(session["state"].value)
        if self.path == "/mark-stale":
            session["state"] = "stale"
            self.respond("DONE!")
        if self.path == "/reset-state":
            session["state"] = "latest"
            self.respond("DONE!")

    def end_headers(self):
        self.send_header('Cross-Origin-Opener-Policy', 'same-origin')
        self.send_header('Cross-Origin-Embedder-Policy', 'require-corp')
        SimpleHTTPRequestHandler.end_headers(self)

if __name__ == '__main__':
    test(RequestHandler, HTTPServer, port=int(sys.argv[1]) if len(sys.argv) > 1 else 8080)

