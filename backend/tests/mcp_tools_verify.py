#!/usr/bin/env python3
"""
MCP Tools Verification — Quick smoke test for the 27 AutoForge MCP tools.

Usage:
    python tests/mcp_tools_verify.py

Requires the AutoForge backend running on http://127.0.0.1:3031/mcp
"""

import json
import sys
import urllib.request
import urllib.error

BASE = "http://127.0.0.1:3031/mcp"


def mcp_call(method: str, params: dict = None, msg_id: int = None) -> tuple[dict, str]:
    """Send a JSON-RPC request over MCP Streamable HTTP.
    Returns (parsed_json_response, session_id)."""
    payload = {"jsonrpc": "2.0", "method": method}
    if msg_id is not None:
        payload["id"] = msg_id
    if params is not None:
        payload["params"] = params

    body = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        BASE,
        data=body,
        headers={
            "Content-Type": "application/json",
            "Accept": "application/json, text/event-stream",
        },
    )

    session_id = None
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            session_id = resp.headers.get("mcp-session-id")
            # SSE parsing: read until we find a data: line with JSON
            buffer = b""
            data_lines = []
            while True:
                chunk = resp.read(1)
                if not chunk:
                    break
                buffer += chunk
                if buffer.endswith(b"\n\n"):
                    text = buffer.decode("utf-8").strip()
                    buffer = b""
                    for line in text.split("\n"):
                        if line.startswith("data:"):
                            data_lines.append(line[5:].strip())
                        elif line.startswith("id:") or line.startswith("retry:"):
                            pass  # SSE control lines
            # The last data line is usually the JSON-RPC response
            for d in reversed(data_lines):
                if d:
                    try:
                        return json.loads(d), session_id
                    except json.JSONDecodeError:
                        continue
            return {}, session_id
    except urllib.error.HTTPError as e:
        print(f"   HTTP {e.code}: {e.read().decode()[:300]}")
        return {}, session_id


def mpc_call_with_session(session_id: str, method: str, params: dict = None, msg_id: int = None) -> dict:
    """Send a JSON-RPC request using an existing session."""
    payload = {"jsonrpc": "2.0", "method": method}
    if msg_id is not None:
        payload["id"] = msg_id
    if params is not None:
        payload["params"] = params

    body = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        BASE,
        data=body,
        headers={
            "Content-Type": "application/json",
            "Accept": "application/json, text/event-stream",
            "mcp-session-id": session_id,
        },
    )

    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            buffer = b""
            data_lines = []
            while True:
                chunk = resp.read(1)
                if not chunk:
                    break
                buffer += chunk
                if buffer.endswith(b"\n\n"):
                    text = buffer.decode("utf-8").strip()
                    buffer = b""
                    for line in text.split("\n"):
                        if line.startswith("data:"):
                            data_lines.append(line[5:].strip())
            for d in reversed(data_lines):
                if d:
                    try:
                        return json.loads(d)
                    except json.JSONDecodeError:
                        continue
            return {}
    except urllib.error.HTTPError as e:
        print(f"   HTTP {e.code}: {e.read().decode()[:300]}")
        return {}


def extract_text(result: dict) -> str:
    """Extract text content from a CallToolResult."""
    content = result.get("content", [])
    if content and isinstance(content, list):
        return content[0].get("text", "")
    return ""


def main():
    print("=" * 60)
    print("MCP Tools Verification")
    print("=" * 60)

    # ------------------------------------------------------------------
    # 1. Initialize
    # ------------------------------------------------------------------
    print("\n[1] Initialize MCP session")
    resp, session_id = mcp_call(
        "initialize",
        {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "verify", "version": "1.0"}},
        msg_id=1,
    )
    if not resp or "result" not in resp:
        print("   [FAIL] Initialize failed")
        return 1
    print(f"   [OK] Session: {session_id}")
    print(f"   Server: {resp['result'].get('serverInfo')}")

    # ------------------------------------------------------------------
    # 2. Send initialized notification
    # ------------------------------------------------------------------
    print("\n[2] Send initialized notification")
    mpc_call_with_session(session_id, "notifications/initialized")
    print("   [OK] Sent")

    # ------------------------------------------------------------------
    # 3. List tools
    # ------------------------------------------------------------------
    print("\n[3] List tools")
    resp = mpc_call_with_session(session_id, "tools/list", msg_id=2)
    tools = resp.get("result", {}).get("tools", [])
    tool_names = [t.get("name") for t in tools]
    print(f"   Found {len(tool_names)} tools")

    expected = [
        "forge_get_project_status",
        "forge_create_session",
        "forge_send_message",
        "forge_list_professions",
        "forge_start_relay_run",
        "forge_list_runs",
        "forge_get_run",
        "forge_read_specs",
        "forge_get_session",
        "forge_list_sessions",
        "forge_delete_session",
        "forge_read_file",
        "forge_browse_directory",
        "forge_approve_spec",
        "forge_reject_spec",
        "forge_list_api_sources",
        "forge_test_api_connection",
        "forge_open_project",
        "forge_close_project",
        "forge_get_performance_logs",
        "forge_batch_start_runs",
        "forge_batch_get_results",
        # New tools
        "forge_poll_chat_status",
        "forge_poll_run_phase",
        "forge_advance_run",
        "forge_submit_handoff",
        "forge_resolve_gate",
    ]

    missing = [e for e in expected if e not in tool_names]
    if missing:
        print(f"   [FAIL] Missing tools: {missing}")
        return 1
    print(f"   [OK] All {len(expected)} expected tools present")

    # ------------------------------------------------------------------
    # 4. forge_get_project_status
    # ------------------------------------------------------------------
    print("\n[4] forge_get_project_status")
    resp = mpc_call_with_session(
        session_id, "tools/call",
        {"name": "forge_get_project_status", "arguments": {}},
        msg_id=3,
    )
    text = extract_text(resp.get("result", {}))
    print(f"   {text[:200]}")

    # ------------------------------------------------------------------
    # 5. forge_create_session
    # ------------------------------------------------------------------
    print("\n[5] forge_create_session")
    resp = mpc_call_with_session(
        session_id, "tools/call",
        {"name": "forge_create_session", "arguments": {"notebook_sid": None, "project_path": "."}},
        msg_id=4,
    )
    text = extract_text(resp.get("result", {}))
    session_data = json.loads(text)
    sid = session_data.get("id")
    print(f"   sid={sid}")

    # ------------------------------------------------------------------
    # 6. forge_get_session (without history)
    # ------------------------------------------------------------------
    print("\n[6] forge_get_session (default, no history)")
    resp = mpc_call_with_session(
        session_id, "tools/call",
        {"name": "forge_get_session", "arguments": {"sid": sid}},
        msg_id=5,
    )
    text = extract_text(resp.get("result", {}))
    data = json.loads(text)
    has_messages_field = "messages" in data
    print(f"   message_count={data.get('message_count')}, has messages field={has_messages_field}")
    assert not has_messages_field, "messages should NOT be present when include_history is omitted"
    print("   [OK] Correctly omits messages field")

    # ------------------------------------------------------------------
    # 7. forge_get_session (with history)
    # ------------------------------------------------------------------
    print("\n[7] forge_get_session (include_history=true)")
    resp = mpc_call_with_session(
        session_id, "tools/call",
        {"name": "forge_get_session", "arguments": {"sid": sid, "include_history": True}},
        msg_id=6,
    )
    text = extract_text(resp.get("result", {}))
    data = json.loads(text)
    msgs = data.get("messages")
    print(f"   message_count={data.get('message_count')}, messages present={msgs is not None}")
    assert msgs is not None, "messages SHOULD be present when include_history=true"
    print("   [OK] Correctly includes messages field")

    # ------------------------------------------------------------------
    # 8. forge_poll_chat_status
    # ------------------------------------------------------------------
    print("\n[8] forge_poll_chat_status")
    resp = mpc_call_with_session(
        session_id, "tools/call",
        {"name": "forge_poll_chat_status", "arguments": {"sid": sid}},
        msg_id=7,
    )
    text = extract_text(resp.get("result", {}))
    data = json.loads(text)
    print(f"   status={data.get('status')}, message_count={data.get('message_count')}")
    assert data.get("sid") == sid
    print("   [OK] Returns correct session status")

    # ------------------------------------------------------------------
    # 9. forge_list_runs
    # ------------------------------------------------------------------
    print("\n[9] forge_list_runs")
    resp = mpc_call_with_session(
        session_id, "tools/call",
        {"name": "forge_list_runs", "arguments": {}},
        msg_id=8,
    )
    text = extract_text(resp.get("result", {}))
    runs = json.loads(text)
    print(f"   {len(runs)} run(s) in store")

    # ------------------------------------------------------------------
    # 10. forge_poll_run_phase (on first run if any)
    # ------------------------------------------------------------------
    print("\n[10] forge_poll_run_phase")
    if runs:
        run_id = runs[0].get("run_id")
        resp = mpc_call_with_session(
            session_id, "tools/call",
            {"name": "forge_poll_run_phase", "arguments": {"run_id": run_id}},
            msg_id=9,
        )
        text = extract_text(resp.get("result", {}))
        data = json.loads(text)
        print(f"   run_id={data.get('run_id')}, status={data.get('status')}, step={data.get('current_step')}/{data.get('total_steps')}")
        print("   [OK] Returns run phase")
    else:
        print("   (no runs to poll — skipping)")

    # ------------------------------------------------------------------
    # 11. forge_delete_session
    # ------------------------------------------------------------------
    print("\n[11] forge_delete_session")
    resp = mpc_call_with_session(
        session_id, "tools/call",
        {"name": "forge_delete_session", "arguments": {"sid": sid}},
        msg_id=10,
    )
    text = extract_text(resp.get("result", {}))
    data = json.loads(text)
    print(f"   deleted={data.get('deleted')}")
    assert data.get("deleted") is True
    print("   [OK] Session deleted")

    # ------------------------------------------------------------------
    # 12. Verify deleted session is gone
    # ------------------------------------------------------------------
    print("\n[12] Verify session is gone")
    resp = mpc_call_with_session(
        session_id, "tools/call",
        {"name": "forge_get_session", "arguments": {"sid": sid}},
        msg_id=11,
    )
    result = resp.get("result", {})
    is_error = result.get("isError") is True
    print(f"   isError={is_error}")
    assert is_error, "Expected error for deleted session"
    print("   [OK] Correctly reports error")

    print("\n" + "=" * 60)
    print("All checks passed [OK]")
    print("=" * 60)
    return 0


if __name__ == "__main__":
    sys.exit(main())
