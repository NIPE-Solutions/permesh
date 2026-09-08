#!/usr/bin/env python3
"""Synthetic draft protocol peer; Permesh does not launch external providers."""
import json
import re
import sys

MAX_FRAME = 1024 * 1024  # Includes trailing LF.
IDENTIFIER = re.compile(r"[A-Za-z][A-Za-z0-9_-]{0,63}\Z")


def emit(out, request_id, event, **data):
    frame = json.dumps({"protocol": 1, "id": request_id, "event": event, **data},
                       separators=(",", ":"), ensure_ascii=True).encode() + b"\n"
    if len(frame) > MAX_FRAME:
        raise ValueError("response exceeds limit")
    out.write(frame)
    out.flush()


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate field")
        result[key] = value
    return result


def discover(out, instance):
    account = {"provider": instance, "id": "alice"}
    resource = {"provider": instance, "id": "repository"}
    group = {"provider": instance, "id": "backend"}
    provenance = {"method": "synthetic fixture", "observed_at": "2026-01-01T00:00:00Z"}
    records = [
        ("identity", {"id": "alice@example.com", "kind": "human", "status": "active",
                      "verified_emails": ["alice@example.com"]}),
        ("account", {"key": account, "login": "alice-dev", "kind": "human",
                     "verified_emails": ["alice@example.com"]}),
        ("resource", {"key": resource, "name": "Synthetic repository"}),
        ("group", {"key": group, "name": "Backend"}),
        ("membership", {"member": {"kind": "account", "key": account}, "group": group,
                        "provenance": provenance}),
        ("grant", {"id": "read", "subject": {"kind": "group", "key": group},
                   "resource": resource, "role": "read", "privilege": "standard",
                   "certainty": "observed", "provenance": provenance}),
    ]
    for kind, data in records:
        emit(out, "discover", "record", kind=kind, data=data)
    emit(out, "discover", "complete", count=len(records), complete=True, limitations=[])


def serve(source, out):
    instance = None
    seen = set()
    while True:
        frame = source.readline(MAX_FRAME + 1)
        if not frame:
            return 0
        if len(frame) > MAX_FRAME or not frame.endswith(b"\n"):
            return 2
        try:
            request = json.loads(frame.decode("utf-8"), object_pairs_hook=unique_object)
        except (ValueError, UnicodeError, RecursionError):
            return 2
        if not isinstance(request, dict):
            return 2
        method = request.get("method")
        expected = {"protocol", "id", "method"}
        if method == "handshake":
            expected.add("instance")
        if (set(request) != expected
                or type(request.get("protocol")) is not int
                or request["protocol"] != 1
                or not isinstance(method, str)
                or method not in {"handshake", "check", "discover", "cancel"}
                or request.get("id") != method):
            return 2
        if method == "handshake" and (
                not isinstance(request["instance"], str)
                or not IDENTIFIER.fullmatch(request["instance"])):
            return 2
        if method in seen:
            emit(out, method, "error", code="protocol_error")
            return 2
        seen.add(method)
        if method != "handshake" and instance is None:
            emit(out, method, "error", code="handshake_required")
            return 2
        if method == "handshake":
            instance = request["instance"]
            emit(out, "handshake", "handshake", provider="synthetic-example",
                 capabilities=["accounts", "identities", "resources", "groups", "memberships", "grants"],
                 draft=True)
        elif method == "cancel":
            emit(out, "cancel", "cancelled")
            return 0
        elif method == "check":
            emit(out, "check", "health", status="ok")
        elif method == "discover":
            discover(out, instance)


if __name__ == "__main__":
    try:
        sys.exit(serve(sys.stdin.buffer, sys.stdout.buffer))
    except (BrokenPipeError, OSError, ValueError):
        # Never echo input, exception details, or environment values.
        sys.exit(2)
