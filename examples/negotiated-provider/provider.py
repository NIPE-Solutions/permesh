#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Standalone synthetic negotiated wire-1 peer. No network, environment or credential use."""
import json
import math
import re
import sys

MAX_FRAME = 1024 * 1024  # Includes LF.
IDENTIFIER = re.compile(r"[A-Za-z][A-Za-z0-9_-]{0,63}\Z")
CAPABILITIES = ["accounts", "identities", "resources", "groups", "memberships", "grants"]


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate field")
        result[key] = value
    return result


def reject_constant(_value):
    raise ValueError("non-JSON number")


def emit(out, request_id, event, **data):
    frame = json.dumps({"protocol_version": 1, "id": request_id, "event": event, **data},
                       separators=(",", ":"), ensure_ascii=True).encode() + b"\n"
    if len(frame) > MAX_FRAME:
        raise ValueError("response too large")
    out.write(frame)
    out.flush()


def read_request(source):
    frame = source.readline(MAX_FRAME + 1)
    if not frame:
        return None
    if len(frame) > MAX_FRAME or not frame.endswith(b"\n"):
        raise ValueError("invalid framing")
    value = json.loads(frame.decode("utf-8"), object_pairs_hook=unique_object,
                       parse_constant=reject_constant)
    if not isinstance(value, dict) or type(value.get("protocol_version")) is not int or value["protocol_version"] != 1:
        raise ValueError("invalid envelope")
    return value


def bounded_json(value, depth=0):
    if depth > 16 or isinstance(value, float) and not math.isfinite(value):
        return False
    if isinstance(value, dict):
        return all(bounded_json(v, depth + 1) for v in value.values())
    if isinstance(value, list):
        return all(bounded_json(v, depth + 1) for v in value)
    return True


def valid_invocation(request, operation):
    if (set(request) != {"protocol_version", "id", "method", "configuration", "credentials"}
            or request["id"] != operation or request["method"] != operation):
        return False
    configuration, credentials = request["configuration"], request["credentials"]
    if (not isinstance(configuration, dict) or not bounded_json(configuration)
            or len(json.dumps(configuration, separators=(",", ":"), ensure_ascii=False).encode()) > 65536
            or not isinstance(credentials, dict) or len(credentials) > 16):
        return False
    total = 0
    for key, value in credentials.items():
        if not IDENTIFIER.fullmatch(key) or not isinstance(value, str):
            return False
        size = len(value.encode())
        if not 1 <= size <= 16384:
            return False
        total += size
    return total <= 65536


def discover(out, instance):
    key = lambda name: {"provider": instance, "id": name}
    provenance = {"method": "synthetic fixture", "observed_at": "2026-01-01T00:00:00Z"}
    records = [
        ("identity", {"id": "robot@example.com", "kind": "service", "affiliation": "external",
                      "status": "inactive", "verified_emails": ["robot@example.com"]}),
        ("account", {"key": key("robot"), "login": "robot", "kind": "service", "affiliation": "external",
                     "status": "inactive", "verified_emails": ["robot@example.com"]}),
        ("resource", {"key": key("organization"), "name": "Synthetic organization",
                      "kind": "example.organization", "parent": None}),
        ("resource", {"key": key("repository"), "name": "Synthetic repository",
                      "kind": "example.repository", "parent": key("organization")}),
        ("group", {"key": key("backend"), "name": "Backend"}),
        ("membership", {"member": {"kind": "account", "key": key("robot")},
                        "group": key("backend"), "provenance": provenance}),
        ("grant", {"id": "read", "subject": {"kind": "group", "key": key("backend")},
                   "resource": key("repository"), "role": "Reader", "privilege": "standard",
                   "certainty": "derived", "evidence_kind": "policy_attachment", "provenance": provenance}),
    ]
    for kind, data in records:
        emit(out, "discover", "record", kind=kind, data=data)
    emit(out, "discover", "complete", count=len(records), complete=True, limitations=[])


def serve(source, out):
    try:
        handshake = read_request(source)
        if (handshake is None or set(handshake) != {"protocol_version", "id", "method", "instance", "operation"}
                or handshake["id"] != "handshake" or handshake["method"] != "handshake"
                or not isinstance(handshake["instance"], str)
                or not IDENTIFIER.fullmatch(handshake["instance"])
                or handshake["operation"] not in ("discover", "check")):
            return 2
        emit(out, "handshake", "handshake", provider="synthetic-example", capabilities=CAPABILITIES,
             operations=["check", "discover"], draft=True)
        request = read_request(source)
        if request == {"protocol_version": 1, "id": "cancel", "method": "cancel"}:
            emit(out, "cancel", "cancelled")
            return 0
        if request is None or not valid_invocation(request, handshake["operation"]):
            return 2
        # Credentials are validated, then discarded; never print request contents.
        del request
        if handshake["operation"] == "check":
            emit(out, "check", "health", status="ok", limitations=[])
        else:
            discover(out, handshake["instance"])
        # The host closes stdin after the single operation. No second invocation.
        return 0 if read_request(source) is None else 2
    except (ValueError, UnicodeError, RecursionError, BrokenPipeError, OSError):
        return 2


if __name__ == "__main__":
    sys.exit(serve(sys.stdin.buffer, sys.stdout.buffer))
