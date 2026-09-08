# SPDX-License-Identifier: MIT
import io
import json
import subprocess
import sys
import unittest
from pathlib import Path

import provider


def handshake(operation="discover"):
    return {"protocol": 5, "id": "handshake", "method": "handshake", "instance": "example",
            "operation": operation}


def invocation(operation="discover"):
    return {"protocol": 5, "id": operation, "method": operation, "configuration": {}, "credentials": {}}


def frames(*values):
    return b"".join(json.dumps(value).encode() + b"\n" for value in values)


class ProviderTests(unittest.TestCase):
    def run_peer(self, data):
        out = io.BytesIO()
        status = provider.serve(io.BytesIO(data), out)
        return status, [json.loads(line) for line in out.getvalue().splitlines()]

    def test_discovery_has_complete_rich_graph(self):
        status, output = self.run_peer(frames(handshake(), invocation()))
        self.assertEqual(status, 0)
        self.assertEqual(output[0]["operations"], ["check", "discover"])
        self.assertEqual(output[0]["capabilities"], provider.CAPABILITIES)
        self.assertTrue(all(frame["protocol"] == 5 for frame in output))
        self.assertEqual(output[-1], {"protocol": 5, "id": "discover", "event": "complete",
                                    "count": 7, "complete": True, "limitations": []})
        records = [frame["data"] for frame in output[1:-1]]
        for entity in records[:2]:
            self.assertEqual((entity["kind"], entity["affiliation"], entity["status"]),
                             ("service", "external", "inactive"))
        self.assertEqual(records[3]["parent"], records[2]["key"])
        self.assertEqual(records[-1]["evidence_kind"], "policy_attachment")
        self.assertEqual(records[-1]["certainty"], "derived")

    def test_health_and_cancel(self):
        status, output = self.run_peer(frames(handshake("check"), invocation("check")))
        self.assertEqual(status, 0)
        self.assertEqual(output[-1]["event"], "health")
        self.assertEqual(output[-1]["status"], "ok")
        self.assertEqual(output[-1]["limitations"], [])
        status, output = self.run_peer(frames(handshake(), {"protocol": 5, "id": "cancel", "method": "cancel"}))
        self.assertEqual(status, 0)
        self.assertEqual(output[-1]["event"], "cancelled")

    def test_rejects_bad_requests_without_echo(self):
        invalid_handshakes = [None, [], {}, {**handshake(), "protocol": True},
                              {**handshake(), "protocol": 2}, {**handshake(), "operation": []}, {**handshake(), "operation": {}},
                              {**handshake(), "operation": "setup"}, {**handshake(), "extra": "private"},
                              {**handshake(), "instance": "../private"}]
        for request in invalid_handshakes:
            with self.subTest(request=request):
                self.assertEqual(self.run_peer(frames(request)), (2, []))
        bad_invocations = [invocation("check"), {**invocation(), "credentials": {"token": ""}},
                           {**invocation(), "credentials": {"token": "x" * 16385}},
                           {**invocation(), "configuration": []}, {**invocation(), "configuration": {"number": float("inf")}},
                           {**invocation(), "credentials": []}, {**invocation(), "credentials": {"9bad": "private"}}, {**invocation(), "extra": "private"}]
        for request in bad_invocations:
            status, output = self.run_peer(frames(handshake(), request))
            self.assertEqual(status, 2)
            self.assertEqual(len(output), 1)
        for data in [b"{}", b"x" * (provider.MAX_FRAME + 1), b"\xff\n",
                     b'{"protocol":5,"protocol":5}\n', b'{"protocol":NaN}\n']:
            self.assertEqual(self.run_peer(data), (2, []))

    def test_credentials_are_never_reflected(self):
        request = invocation()
        request["credentials"] = {"token": "SENTINEL_PRIVATE"}
        process = subprocess.run([sys.executable, str(Path(provider.__file__))],
                                 input=frames(handshake(), request), capture_output=True, timeout=5)
        self.assertEqual(process.returncode, 0)
        self.assertEqual(process.stderr, b"")
        self.assertNotIn(b"SENTINEL_PRIVATE", process.stdout)

    def test_repeated_operation_and_truncated_session_fail(self):
        self.assertEqual(self.run_peer(frames(handshake()))[0], 2)
        self.assertEqual(self.run_peer(frames(handshake(), invocation(), invocation()))[0], 2)


if __name__ == "__main__":
    unittest.main()
