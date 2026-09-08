#!/usr/bin/env python3
"""Synthetic draft protocol peer; Permesh does not launch external providers."""
import json
import sys

MAX_FRAME = 1024 * 1024  # Includes trailing LF.


def emit(out, request_id, event, **data):
    frame = json.dumps({"protocol": 1, "id": request_id, "event": event, **data},
                       separators=(",", ":"), ensure_ascii=True).encode() + b"\n"
    if len(frame) > MAX_FRAME:
        raise ValueError("response exceeds limit")
    out.write(frame)
    out.flush()


def serve(source, out):
    handshaken = False
    while True:
        frame = source.readline(MAX_FRAME + 1)
        if not frame:
            return 0
        if len(frame) > MAX_FRAME or not frame.endswith(b"\n"):
            return 2
        try:
            request = json.loads(frame.decode("utf-8"))
        except (ValueError, UnicodeError, RecursionError):
            return 2
        if not isinstance(request, dict):
            return 2
        request_id = request.get("id")
        if (set(request) != {"protocol", "id", "method"}
                or type(request.get("protocol")) is not int
                or request["protocol"] != 1
                or not isinstance(request_id, str)
                or not 1 <= len(request_id) <= 64
                or not request_id.isascii()
                or not request_id.replace("-", "").replace("_", "").isalnum()
                or not isinstance(request.get("method"), str)):
            return 2
        method = request["method"]
        if method == "handshake" and not handshaken:
            handshaken = True
            emit(out, request_id, "handshake", provider="synthetic-example",
                 capabilities=["check", "discover"], draft=True)
        elif method == "cancel":
            emit(out, request_id, "cancelled")
            return 0
        elif not handshaken:
            emit(out, request_id, "error", code="handshake_required")
        elif method == "check":
            emit(out, request_id, "health", status="ok", synthetic=True)
        elif method == "discover":
            emit(out, request_id, "record", kind="account", data={
                "id": "synthetic-example:alice", "name": "Alice Example",
                "email": "alice@example.invalid", "verified_email": False})
            emit(out, request_id, "record", kind="resource", data={
                "id": "synthetic-example:repo", "name": "Synthetic repository"})
            emit(out, request_id, "record", kind="grant", data={
                "id": "synthetic-example:grant", "subject": "synthetic-example:alice",
                "resource": "synthetic-example:repo", "permission": "read"})
            emit(out, request_id, "complete", count=3, synthetic=True)
        else:
            emit(out, request_id, "error", code="unsupported_method")


if __name__ == "__main__":
    try:
        sys.exit(serve(sys.stdin.buffer, sys.stdout.buffer))
    except (BrokenPipeError, OSError, ValueError):
        # Never echo input, exception details, or environment values.
        sys.exit(2)
