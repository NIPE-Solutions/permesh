import io
import json
import pathlib
import subprocess
import sys
import unittest

from provider import MAX_FRAME, serve


def request(method, **extra):
    data = {"protocol": 1, "id": method, "method": method}
    if method == "handshake":
        data["instance"] = "example-main"
    data.update(extra)
    return json.dumps(data).encode() + b"\n"


class ProtocolTests(unittest.TestCase):
    def run_peer(self, payload):
        out = io.BytesIO()
        code = serve(io.BytesIO(payload), out)
        return code, [json.loads(line) for line in out.getvalue().splitlines()]

    def test_handshake_check_discover(self):
        code, events = self.run_peer(request("handshake") + request("check") + request("discover"))
        self.assertEqual(code, 0)
        self.assertEqual([e["event"] for e in events],
                         ["handshake", "health"] + ["record"] * 6 + ["complete"])
        self.assertEqual(events[0]["capabilities"],
                         ["accounts", "identities", "resources", "groups", "memberships", "grants"])
        self.assertEqual(events[1], {"protocol": 1, "id": "check", "event": "health", "status": "ok"})
        self.assertEqual(events[-1], {"protocol": 1, "id": "discover", "event": "complete",
                                     "count": 6, "complete": True, "limitations": []})
        identity, account, resource, group, membership, grant = [e["data"] for e in events[2:-1]]
        self.assertEqual(identity["verified_emails"], account["verified_emails"])
        self.assertEqual(membership["member"], {"kind": "account", "key": account["key"]})
        self.assertEqual(membership["group"], group["key"])
        self.assertEqual(grant["subject"], {"kind": "group", "key": group["key"]})
        self.assertEqual(grant["resource"], resource["key"])
        self.assertEqual(grant["privilege"], "standard")
        self.assertEqual(grant["certainty"], "observed")

    def test_dynamic_instance(self):
        for instance in ("A", "foo_B-9", "a" * 64):
            code, events = self.run_peer(request("handshake", instance=instance) + request("discover"))
            self.assertEqual(code, 0)
            account, resource, group = [events[i]["data"] for i in (2, 3, 4)]
            self.assertEqual([v["key"]["provider"] for v in (account, resource, group)], [instance] * 3)
            self.assertEqual(events[5]["data"]["member"]["key"], account["key"])
            self.assertEqual(events[6]["data"]["resource"], resource["key"])

    def test_requires_handshake(self):
        for method in ("check", "discover", "cancel"):
            code, events = self.run_peer(request(method))
            self.assertEqual(code, 2)
            self.assertEqual(events, [{"protocol": 1, "id": method, "event": "error", "code": "handshake_required"}])

    def test_cancel_exits_without_processing_more_input(self):
        code, events = self.run_peer(request("handshake") + request("cancel") + b"secret-sentinel\n")
        self.assertEqual(code, 0)
        self.assertEqual(events[-1], {"protocol": 1, "id": "cancel", "event": "cancelled"})
        self.assertEqual(len(events), 2)

    def test_malformed_and_oversized_input_is_not_echoed(self):
        for payload in [b"secret-sentinel\n", b"[]\n", b"\xff\n", request("check")[:-1],
                        b"x" * MAX_FRAME + b"\n", request("check", protocol=True),
                        request("check", id="secret-sentinel"), request("check", path="secret-sentinel"),
                        b'{"protocol":1,"protocol":1,"id":"check","method":"check"}\n',
                        b'{"protocol":1,"id":"check","method":"check","extra":{"x":1,"x":2}}\n']:
            with self.subTest(payload_length=len(payload)):
                self.assertEqual(self.run_peer(payload), (2, []))

    def test_invalid_instances(self):
        for instance in ("", "a" * 65, "1foo", "_foo", "é", "a/b", "$(pwd)", None, True):
            with self.subTest(instance=instance):
                self.assertEqual(self.run_peer(request("handshake", instance=instance)), (2, []))
        self.assertEqual(self.run_peer(b'{"protocol":1,"id":"handshake","method":"handshake"}\n'), (2, []))

    def test_repeated_requests_fail_without_more_records(self):
        for method in ("handshake", "check", "discover"):
            prefix = request("handshake") if method != "handshake" else b""
            code, events = self.run_peer(prefix + request(method) + request(method) + request("cancel"))
            self.assertEqual(code, 2)
            self.assertEqual(events[-1], {"protocol": 1, "id": method, "event": "error", "code": "protocol_error"})
            self.assertEqual(sum(e["event"] == "complete" for e in events), int(method == "discover"))

    def test_unknown_method_and_request_id_are_not_reflected(self):
        code, events = self.run_peer(request("handshake") + request("SENTINEL_PRIVATE_TOKEN"))
        self.assertEqual(code, 2)
        self.assertEqual(len(events), 1)
        self.assertEqual(events[0]["event"], "handshake")
        self.assertNotIn("SENTINEL_PRIVATE_TOKEN", json.dumps(events))

    def test_bounded_reads_and_exact_limit(self):
        class BoundedSource(io.BytesIO):
            def readline(self, size=-1):
                self.assert_size(size)
                return super().readline(size)
        source = BoundedSource(b"x" * (MAX_FRAME + 10))
        source.assert_size = lambda size: self.assertEqual(size, MAX_FRAME + 1)
        self.assertEqual(serve(source, io.BytesIO()), 2)
        self.assertEqual(source.tell(), MAX_FRAME + 1)
        base = request("handshake")
        self.assertEqual(self.run_peer(base[:-1] + b" " * (MAX_FRAME - len(base)) + b"\n")[0], 0)
        self.assertEqual(self.run_peer(base[:-1] + b" " * (MAX_FRAME - len(base) + 1) + b"\n"), (2, []))

    def test_real_process_matches_fixture(self):
        proc = subprocess.run([sys.executable, str(pathlib.Path(__file__).with_name("provider.py"))],
                              input=request("handshake") + request("discover"), capture_output=True,
                              timeout=5, check=True)
        self.assertEqual(proc.stderr, b"")
        self.assertEqual(proc.stdout, pathlib.Path(__file__).with_name("discovery.ndjson").read_bytes())
        self.assertTrue(proc.stdout.isascii())
        self.assertEqual(len(proc.stdout.splitlines()), 8)


if __name__ == "__main__":
    unittest.main()
