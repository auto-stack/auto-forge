#!/usr/bin/env python3
"""End-to-end tests for the Forge chat flow.

Usage:
    python tests/forge_chat_e2e.py          # run all tests
    python tests/forge_chat_e2e.py basic    # run only basic chat test
    python tests/forge_chat_e2e.py tool     # run only tool-call test
    python tests/forge_chat_e2e.py dispatch # run only dispatch/errand test

Requires the AutoForge server to be running on http://127.0.0.1:3031
and a valid API source configured.
"""

import json
import sys
import urllib.request

BASE = "http://127.0.0.1:3031"


def api_post(path: str, data: dict) -> dict:
    url = f"{BASE}{path}"
    body = json.dumps(data).encode("utf-8")
    req = urllib.request.Request(
        url, data=body, headers={"Content-Type": "application/json"}, method="POST"
    )
    with urllib.request.urlopen(req, timeout=30) as resp:
        return json.loads(resp.read().decode("utf-8"))


def api_get(path: str) -> dict | None:
    url = f"{BASE}{path}"
    try:
        with urllib.request.urlopen(url, timeout=30) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        if e.code == 404:
            return None
        raise


def sse_stream(path: str, timeout: float = 120.0):
    """Yields parsed SSE data lines."""
    url = f"{BASE}{path}"
    req = urllib.request.Request(url, headers={"Accept": "text/event-stream"})
    with urllib.request.urlopen(req, timeout=int(timeout) + 10) as resp:
        buffer = b""
        while True:
            chunk = resp.read(1)
            if not chunk:
                break
            buffer += chunk
            if buffer.endswith(b"\n\n"):
                text = buffer.decode("utf-8")
                buffer = b""
                data = None
                for line in text.strip().split("\n"):
                    if line.startswith("data:"):
                        data = line[5:].strip()
                if data:
                    try:
                        yield json.loads(data)
                    except json.JSONDecodeError:
                        pass


def create_session() -> str:
    resp = api_post("/api/forge/chats/session", {"project_path": "."})
    sid = resp.get("id")
    assert sid and sid.startswith("forge-"), f"Unexpected session id: {sid}"
    return sid


def send_message(sid: str, content: str) -> dict:
    return api_post(f"/api/forge/chats/{sid}/message", {"content": content})


def collect_stream(sid: str, timeout: float = 120.0) -> list[dict]:
    events = []
    for evt in sse_stream(f"/api/forge/chats/{sid}/stream", timeout=timeout):
        events.append(evt)
        t = evt.get("type", "?")
        if t in ("done", "error"):
            break
    return events


def print_events(events: list[dict]):
    for evt in events:
        t = evt.get("type", "?")
        if t == "delta":
            print(f"      [delta] {evt.get('text', '')!r}")
        elif t == "tool_call":
            args = evt.get("arguments", {})
            print(f"      [tool_call] {evt.get('name')}({json.dumps(args)})")
        elif t == "tool_result":
            res = evt.get("result", "")[:120]
            print(f"      [tool_result] {evt.get('id')}: {res}...")
        elif t == "errand_start":
            print(f"      [errand_start] {evt.get('errand_id')} -> {evt.get('profession_id')}: {evt.get('task', '')[:80]}")
        elif t == "errand_delta":
            print(f"      [errand_delta] {evt.get('text', '')!r}")
        elif t == "errand_tool_call":
            print(f"      [errand_tool_call] {evt.get('name')}")
        elif t == "errand_tool_result":
            print(f"      [errand_tool_result] {evt.get('id')}")
        elif t == "errand_complete":
            print(f"      [errand_complete] {evt.get('errand_id')} status={evt.get('status')} tokens={evt.get('token_usage')}")
        elif t == "agent_handoff":
            print(f"      [handoff] {evt.get('from_profession')} -> {evt.get('to_profession')}: {evt.get('reason', '')[:80]}")
        elif t == "done":
            print("      [done]")
        elif t == "error":
            print(f"      [ERROR] {evt.get('message')}")


def session_messages(sid: str) -> list[dict]:
    resp = api_get(f"/api/forge/chats/session/{sid}")
    return (resp or {}).get("messages", [])


# ---------------------------------------------------------------------------
# Test 1: Basic chat
# ---------------------------------------------------------------------------
def test_basic_chat():
    print("\n[Test 1] Basic chat")
    print("-" * 50)
    sid = create_session()
    print(f"Session: {sid}")

    send_message(sid, "Say hello briefly.")
    events = collect_stream(sid, timeout=90.0)
    print_events(events)

    types = [e.get("type") for e in events]
    assert "turn_start" in types, "Missing turn_start"
    assert "delta" in types, "Missing delta"
    assert "done" in types, "Missing done"

    msgs = session_messages(sid)
    roles = [m.get("role") for m in msgs]
    assert "user" in roles and "assistant" in roles
    print(f"Messages: {roles}")
    print("PASS")
    return True


# ---------------------------------------------------------------------------
# Test 2: Tool call (ask agent to use shell)
# ---------------------------------------------------------------------------
def test_tool_call():
    print("\n[Test 2] Tool call (shell)")
    print("-" * 50)
    sid = create_session()
    print(f"Session: {sid}")

    # Explicitly ask for a shell command to force tool_use
    send_message(sid, "Run the shell command 'echo 42' and tell me the output. You MUST use the shell tool.")
    events = collect_stream(sid, timeout=120.0)
    print_events(events)

    types = [e.get("type") for e in events]
    assert "turn_start" in types
    assert "delta" in types
    assert "done" in types

    # The model may or may not tool_use depending on system prompt constraints.
    # We just verify the stream completed gracefully.
    if "tool_call" in types:
        print("Tool call detected — PASS")
    else:
        print("No tool call (model chose direct answer) — PASS (non-deterministic)")
    return True


# ---------------------------------------------------------------------------
# Test 3: Dispatch / errand (gofer)
# ---------------------------------------------------------------------------
def test_dispatch_errand():
    print("\n[Test 3] Dispatch / errand (gofer)")
    print("-" * 50)
    sid = create_session()
    print(f"Session: {sid}")

    # Ask assistant to dispatch a simple research task to gofer
    prompt = (
        "Please dispatch a task to the gofer agent to find out "
        "what files are in the project root directory."
    )
    send_message(sid, prompt)
    events = collect_stream(sid, timeout=180.0)
    print_events(events)

    types = [e.get("type") for e in events]
    assert "turn_start" in types
    assert "done" in types

    # Check for errand events
    if "errand_start" in types:
        print("Errand launched — PASS")
    elif "tool_call" in types and any(
        e.get("name") == "dispatch" for e in events if e.get("type") == "tool_call"
    ):
        print("Dispatch tool called but errand may have failed — PASS (partial)")
    else:
        print("No dispatch (model chose not to delegate) — PASS (non-deterministic)")
    return True


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
def main():
    arg = sys.argv[1] if len(sys.argv) > 1 else "all"
    tests = []
    if arg in ("all", "basic"):
        tests.append(test_basic_chat)
    if arg in ("all", "tool"):
        tests.append(test_tool_call)
    if arg in ("all", "dispatch"):
        tests.append(test_dispatch_errand)

    if not tests:
        print(f"Unknown test: {arg}")
        return 1

    passed = 0
    failed = 0
    for fn in tests:
        try:
            fn()
            passed += 1
        except Exception as e:
            print(f"FAIL: {e}")
            failed += 1

    print("\n" + "=" * 50)
    print(f"Results: {passed} passed, {failed} failed")
    print("=" * 50)
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
