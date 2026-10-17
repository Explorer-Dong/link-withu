"""Startup transport failures must retry without hiding failed application checks."""

import os
import socket
import struct
import threading
import unittest
from http.client import IncompleteRead, RemoteDisconnected
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from unittest.mock import patch
from urllib.error import URLError

import smoke


class SmokeReadiness(unittest.TestCase):
    def test_retries_startup_transport_failures(self):
        health = {"status": "ok", "version": "1.0.1"}
        failures = [
            URLError("connection refused"),
            ConnectionResetError(104, "Connection reset by peer"),
            RemoteDisconnected("Remote end closed connection"),
            IncompleteRead(b"{"),
            TimeoutError("timed out"),
        ]
        with (
            patch.object(smoke, "request", side_effect=[*failures, health]) as request,
            patch.object(smoke.time, "sleep") as sleep,
        ):
            self.assertEqual(smoke.wait_for_health(), health)
        self.assertEqual(request.call_count, len(failures) + 1)
        self.assertEqual(sleep.call_count, len(failures))

    def test_real_http_connection_reset_then_success(self):
        class Handler(BaseHTTPRequestHandler):
            attempts = 0

            def do_GET(self):
                type(self).attempts += 1
                if self.attempts == 1:
                    self.connection.setsockopt(
                        socket.SOL_SOCKET, socket.SO_LINGER, struct.pack("ii", 1, 0)
                    )
                    self.close_connection = True
                    self.connection.close()
                    return
                body = b'{"status":"ok","version":"1.0.1"}'
                self.send_response(200)
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *_args):
                pass

        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        try:
            with patch.object(smoke, "BASE", f"http://127.0.0.1:{server.server_port}"):
                self.assertEqual(smoke.wait_for_health(timeout=5)["status"], "ok")
            self.assertEqual(Handler.attempts, 2)
        finally:
            server.shutdown()
            server.server_close()
            worker.join(timeout=5)

    def test_persistent_failure_has_a_deadline_and_preserves_cause(self):
        failure = ConnectionResetError(104, "Connection reset by peer")
        with (
            patch.object(smoke, "request", side_effect=failure),
            patch.object(smoke.time, "monotonic", side_effect=[100, 101]),
            self.assertRaisesRegex(RuntimeError, "did not become ready") as caught,
        ):
            smoke.wait_for_health(timeout=1)
        self.assertIs(caught.exception.__cause__, failure)

    def test_invalid_response_is_not_treated_as_a_startup_delay(self):
        with (
            patch.object(smoke, "request", side_effect=ValueError("invalid JSON")),
            self.assertRaises(ValueError),
        ):
            smoke.wait_for_health()

    def test_ready_server_with_wrong_version_still_fails(self):
        with (
            patch.dict(os.environ, {"EXPECTED_VERSION": "1.0.1"}),
            patch.object(smoke, "wait_for_health", return_value={"status": "ok", "version": "1.0.0"}),
            self.assertRaises(AssertionError),
        ):
            smoke.main()


if __name__ == "__main__":
    unittest.main()
