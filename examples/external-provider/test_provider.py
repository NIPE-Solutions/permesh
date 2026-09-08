import io
import json
import pathlib
import subprocess
import sys
import unittest

from provider import MAX_FRAME, serve


def request(method, request_id="one"):
    return json.dumps({"protocol": 1, "id": request_id, "method": method}).encode() + b"\n"


class ProtocolTests(unittest.TestCase):
    def run_peer(self, payload):
        out = io.BytesIO()
        code = serve(io.BytesIO(payload), out)
        return code, [json.loads(line) for line in out.getvalue().splitlines()]

    def test_handshake_check_discover(self):
        code, events = self.run_peer(request("handshake") + request("check") + request("discover"))
        self.assertEqual(code, 0)
        self.assertEqual([e["event"] for e in events],
                         ["handshake", "health", "record", "record", "record", "complete"])
        self.assertEqual(events[-1]["count"], 3)
        grant = events[-2]["data"]
        self.assertEqual(grant["subject"], events[2]["data"]["id"])
        self.assertEqual(grant["resource"], events[3]["data"]["id"])

    def test_requires_handshake(self):
        self.assertEqual(self.run_peer(request("discover"))[1][0]["code"], "handshake_required")

    def test_cancel_exits_without_processing_more_input(self):
        self.assertEqual(len(self.run_peer(request("cancel") + request("handshake"))[1]), 1)

    def test_malformed_and_oversized_input_is_not_echoed(self):
        for payload in [b"secret-sentinel\n", b"[]\n", b"\xff\n", request("check")[:-1],
                        b"x" * MAX_FRAME + b"\n", b'{"protocol":true,"id":"x","method":"check"}\n']:
            with self.subTest(payload_length=len(payload)):
                self.assertEqual(self.run_peer(payload), (2, []))

    def test_real_process_ndjson(self):
        proc = subprocess.run([sys.executable, str(pathlib.Path(__file__).with_name("provider.py"))],
                              input=request("handshake"), capture_output=True, timeout=5, check=True)
        self.assertEqual(proc.stderr, b"")
        self.assertTrue(json.loads(proc.stdout)["draft"])


if __name__ == "__main__":
    unittest.main()
