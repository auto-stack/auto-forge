#!/usr/bin/env python3
"""
Mock Relay Test — Fast, zero-LLM-cost validation of the relay pipeline.

Creates a relay run via the API and manually drives it through steps with
synthetic handoffs. This tests:
  - Pipeline engine state machine
  - Monitoring UI (Relay view)
  - Gate resolution flow
  - Step history and token tracking

Run with: python backend/tests/forge_relay_mock.py
Requires: backend server running on http://127.0.0.1:3031
"""

import urllib.request
import urllib.error
import json
import time
import sys

BASE = "http://127.0.0.1:3031"


def api_call(method, path, data=None):
    """Make a JSON API call and return parsed response."""
    url = f"{BASE}{path}"
    body = json.dumps(data).encode("utf-8") if data else None
    req = urllib.request.Request(url, data=body, method=method)
    if body:
        req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        print(f"   HTTP {e.code}: {e.read().decode()[:200]}")
        return None
    except Exception as e:
        print(f"   Error: {e}")
        return None


def create_run(flow_id="post_discovery"):
    """Create a relay run via the public API."""
    flows = {
        "post_discovery": [
            {"id": "design", "profession_id": "architect", "gate": "auto"},
            {"id": "plan", "profession_id": "planner", "gate": "auto"},
            {"id": "draft-tests", "profession_id": "tester", "gate": "auto"},
            {"id": "code", "profession_id": "coder", "gate": "auto"},
            {"id": "run-tests", "profession_id": "tester", "gate": "auto"},
            {"id": "review", "profession_id": "reviewer", "gate": "auto"},
            {"id": "report", "profession_id": "documenter", "gate": "auto"},
        ],
        "standard": [
            {"id": "intake", "profession_id": "assistant", "gate": "auto"},
            {"id": "discover", "profession_id": "advisor", "gate": "human"},
            {"id": "design", "profession_id": "architect", "gate": "auto"},
            {"id": "plan", "profession_id": "planner", "gate": "auto"},
            {"id": "draft-tests", "profession_id": "tester", "gate": "auto"},
            {"id": "code", "profession_id": "coder", "gate": "auto"},
            {"id": "run-tests", "profession_id": "tester", "gate": "auto"},
            {"id": "review", "profession_id": "reviewer", "gate": "auto"},
            {"id": "report", "profession_id": "documenter", "gate": "auto"},
        ],
    }
    steps = flows.get(flow_id, flows["post_discovery"])

    result = api_call("POST", "/api/forge/relay/runs", {"flow_id": flow_id, "steps": steps})
    if not result:
        raise RuntimeError("Failed to create run")
    return result["run_id"]


def get_run(run_id):
    return api_call("GET", f"/api/forge/relay/runs/{run_id}")


def advance(run_id):
    return api_call("POST", f"/api/forge/relay/runs/{run_id}/advance")


def submit_handoff(run_id, profession_id, summary):
    """Submit a handoff with validation-aware data for each profession.

    Supports BOTH flow-config validators AND fallback hardcoded validators.
    """
    spec_updates = []
    work_product = []
    decisions = []

    # Add data to pass step validators (both new StepValidator and fallback)
    if profession_id == "advisor":
        # Fallback: needs spec_updates OR work_product
        # StepValidator: needs spec_updates for "goals" OR work_product with .ad
        spec_updates.append({"section_id": "goals", "item_id": None, "change_type": "modified", "description": "Updated goals"})
        work_product.append({"path": "specs/goals.ad", "description": "Goals updated", "lines": None})
    elif profession_id == "architect":
        # Fallback: needs meaningful work_product (not README.md)
        # StepValidator: needs spec_updates for "architecture"/"designs" OR decisions
        spec_updates.append({"section_id": "architecture", "item_id": None, "change_type": "modified", "description": "Updated architecture"})
        work_product.append({"path": "specs/architecture.ad", "description": "Architecture doc", "lines": None})
    elif profession_id == "planner":
        # Fallback: needs plan-related work_product OR >=2 work products
        # StepValidator: needs spec_updates for "plans"
        spec_updates.append({"section_id": "plans", "item_id": None, "change_type": "modified", "description": "Updated plans"})
        work_product.append({"path": "specs/plans.ad", "description": "Plan doc", "lines": None})
    elif profession_id == "tester":
        # Fallback + StepValidator: needs .rs/.ts/.vue work_product OR tests spec_updates
        work_product.append({"path": "src/lib.rs", "description": "Tests", "lines": None})
    elif profession_id == "coder":
        # Fallback + StepValidator: needs .rs/.vue/.ts/.js work_product
        work_product.append({"path": "src/main.rs", "description": "Code", "lines": None})
    elif profession_id == "reviewer":
        # Fallback + StepValidator: needs decisions OR reviews spec_updates
        decisions.append({"id": "D1", "title": "Approved", "status": "made", "rationale": ""})
        work_product.append({"path": "specs/reviews.ad", "description": "Review", "lines": None})
    elif profession_id == "documenter":
        # Fallback + StepValidator: needs .md/.ad work_product OR reports spec_updates
        work_product.append({"path": "docs/report.md", "description": "Report", "lines": None})
    elif profession_id == "assistant":
        work_product.append({"path": "notes.txt", "description": "Notes", "lines": None})

    handoff = {
        "from": profession_id,
        "to": "next",
        "run_id": run_id,
        "checkpoint_id": 0,
        "summary": summary,
        "decisions": decisions,
        "open_questions": [],
        "spec_updates": spec_updates,
        "work_product": work_product,
        "context_for_next": {"files_to_read": [], "specs_to_follow": [], "warnings": []},
        "token_usage": {
            "step_input": 1000,
            "step_output": 500,
            "cumulative": 1500,
            "budget_remaining": 98500
        }
    }
    return api_call("POST", f"/api/forge/relay/runs/{run_id}/handoff", {"handoff": handoff})


def resolve_gate(run_id, decision="approve", feedback=""):
    body = {"decision": decision}
    if feedback:
        body["feedback"] = feedback
    return api_call("POST", f"/api/forge/relay/runs/{run_id}/gate", body)


def drive_run(run_id, step_summaries):
    """Manually drive a run to completion."""
    print(f"\nDriving run {run_id}...")
    for i in range(30):  # safety limit
        state = get_run(run_id)
        if not state:
            print("FAIL Run not found!")
            return False

        status = state["status"]
        step_idx = state["current_step"]
        total = state["total_steps"]

        if status == "Completed":
            print(f"\nOK Relay completed at step {step_idx}/{total}")
            print(f"   Total tokens: {state.get('cumulative_tokens', 0)}")
            return True

        if status.startswith("Failed"):
            print(f"\nFAIL Relay failed: {status}")
            return False

        if state.get("waiting_for_gate"):
            gate = state["waiting_for_gate"]
            print(f"||  Gate waiting at {gate['step_id']} ({gate['profession_id']})")
            print("   → Approving gate...")
            resolve_gate(run_id, "approve")
            continue

        # Advance and submit synthetic handoff
        result = advance(run_id)
        if result and "ExecuteStep" in result.get("result", ""):
            # Extract profession from the debug string
            prof = "unknown"
            result_str = result.get("result", "")
            if "profession_id:" in result_str:
                try:
                    prof = result_str.split('profession_id: "')[1].split('"')[0]
                except IndexError:
                    pass

            step_id = state["steps"][min(step_idx, len(state["steps"]) - 1)]["id"]
            summary = step_summaries.get(step_id, f"Completed {step_id} step.")
            print(f">>  Step {step_idx + 1}/{total}: {prof} — {summary[:60]}...")
            submit_handoff(run_id, prof, summary)
        else:
            print(f"   Advance result: {result}")

        time.sleep(0.3)

    print("\nWARN  Hit safety limit — run may still be in progress")
    return False


def test_post_discovery_flow():
    """Test the post-discovery flow (no gates in GSD mode)."""
    print("=" * 55)
    print("Test 1: Post-Discovery Flow (GSD, no gates)")
    print("=" * 55)

    run_id = create_run("post_discovery")
    print(f"Created run: {run_id}")

    summaries = {
        "design": "Designed a modular caching layer with Redis backend and TTL support.",
        "plan": "Planned 3 milestones: interface, implementation, integration tests.",
        "draft-tests": "Drafted 12 test cases covering hit/miss, expiry, and eviction.",
        "code": "Implemented CacheManager with get/set/delete and async operations.",
        "run-tests": "All 12 tests pass. Coverage: 94%.",
        "review": "Code reviewed: clean architecture, good error handling, minor doc fixes suggested.",
        "report": "Delivered caching module with full test coverage and documentation.",
    }

    ok = drive_run(run_id, summaries)

    state = get_run(run_id)
    print(f"\nFinal state:\n{json.dumps(state, indent=2)}")
    return ok


def test_standard_flow_with_gate():
    """Test the standard flow where the advisor gate pauses in GSD mode."""
    print("\n" + "=" * 55)
    print("Test 2: Standard Flow (GSD, advisor gate)")
    print("=" * 55)

    run_id = create_run("standard")
    print(f"Created run: {run_id}")

    summaries = {
        "intake": "User wants a caching system for the API layer.",
        "discover": "Goals clarified: G1 - Get/set/delete, G2 - TTL expiry, G3 - 95% test coverage.",
        "design": "Designed modular cache with pluggable backends.",
        "plan": "3-sprint plan: interface → impl → tests → docs.",
        "draft-tests": "12 tests drafted.",
        "code": "CacheManager implemented.",
        "run-tests": "All tests pass.",
        "review": "Approved with minor docs update.",
        "report": "Caching module delivered.",
    }

    ok = drive_run(run_id, summaries)

    state = get_run(run_id)
    print(f"\nFinal state:\n{json.dumps(state, indent=2)}")
    return ok


def test_reject_and_retry():
    """Test gate rejection and retry loop."""
    print("\n" + "=" * 55)
    print("Test 3: Gate Reject + Retry")
    print("=" * 55)

    run_id = create_run("standard")
    print(f"Created run: {run_id}")

    # Drive to the advisor gate
    for i in range(10):
        state = get_run(run_id)
        if state.get("waiting_for_gate"):
            break
        result = advance(run_id)
        if result and "ExecuteStep" in result.get("result", ""):
            # Submit handoff to move forward
            prof = "assistant" if state["current_step"] == 0 else "advisor"
            submit_handoff(run_id, prof, f"Step {state['current_step']} completed.")
        time.sleep(0.2)

    state = get_run(run_id)
    if state.get("waiting_for_gate"):
        gate = state["waiting_for_gate"]
        print(f"||  Gate at {gate['step_id']} — rejecting with feedback")
        resolve_gate(run_id, "reject", "Goals are too vague. Need concrete performance targets.")
        time.sleep(0.2)

        # Should be back at discover step
        state = get_run(run_id)
        print(f"   Status after reject: {state['status']}, step: {state['current_step']}")

        # Submit revised handoff
        if "ExecuteStep" in str(advance(run_id)):
            submit_handoff(run_id, "advisor", "Revised goals with concrete performance targets.")
            time.sleep(0.2)

        # Approve on second attempt
        state = get_run(run_id)
        if state.get("waiting_for_gate"):
            print("   Approving on retry...")
            resolve_gate(run_id, "approve")
            time.sleep(0.2)

        # Continue to completion
        summaries = {
            "design": "Design approved.",
            "plan": "Plan approved.",
            "draft-tests": "Tests drafted.",
            "code": "Code done.",
            "run-tests": "Tests pass.",
            "review": "Review passed.",
            "report": "Report done.",
        }
        drive_run(run_id, summaries)

    print(f"\nFinal state:\n{json.dumps(get_run(run_id), indent=2)}")
    return True


def test_goal_discovery_flow():
    """Test the goal-discovery flow where advisor writes goals."""
    print("\n" + "=" * 55)
    print("Test 4: Goal-Discovery Flow (advisor writes goals)")
    print("=" * 55)

    run_id = create_run("goal-discovery")
    print(f"Created run: {run_id}")

    # Drive the single discover step
    ok = drive_run(run_id, {
        "discover": "Goals written: G1 - Add caching layer, G2 - Implement TTL support.",
    })

    state = get_run(run_id)
    print(f"\nFinal state:\n{json.dumps(state, indent=2)}")
    return ok


def test_professions_have_write_goals():
    """Verify advisor profession has write_goals in allowed_tools."""
    print("\n" + "=" * 55)
    print("Test 5: Professions config")
    print("=" * 55)

    result = api_call("GET", "/api/forge/relay/professions")
    if not result:
        print("FAIL Failed to fetch professions")
        return False

    advisor = next((p for p in result.get("professions", []) if p["id"] == "advisor"), None)
    if not advisor:
        print("FAIL Advisor profession not found")
        return False

    if "write_goals" in advisor.get("allowed_tools", []):
        print("OK Advisor has write_goals tool")
        return True
    else:
        print(f"FAIL Advisor missing write_goals. Tools: {advisor.get('allowed_tools')}")
        return False


def test_flow_registry_has_goal_discovery():
    """Verify goal-discovery flow is available."""
    print("\n" + "=" * 55)
    print("Test 6: Flow Registry")
    print("=" * 55)

    # Create a run with flow_id only (no steps) — requires registry lookup
    result = api_call("POST", "/api/forge/relay/runs", {
        "flow_id": "goal-discovery",
        "task": "Test goal discovery flow",
    })
    if not result:
        print("FAIL Failed to create run from registry")
        return False

    run_id = result["run_id"]
    state = result["state"]
    print(f"OK Created run {run_id} with {state['total_steps']} step(s)")

    # Cleanup
    api_call("DELETE", f"/api/forge/relay/runs/{run_id}")
    return True


def main():
    # Health check
    try:
        result = api_call("GET", "/api/forge/relay/runs")
        if result is None:
            raise RuntimeError("No response")
    except Exception as e:
        print(f"FAIL Backend not reachable at {BASE}: {e}")
        print("   Please start the server: cd backend && cargo run")
        sys.exit(1)

    results = []
    results.append(("Post-Discovery Flow", test_post_discovery_flow()))
    results.append(("Standard Flow + Gate", test_standard_flow_with_gate()))
    results.append(("Reject + Retry", test_reject_and_retry()))
    results.append(("Goal-Discovery Flow", test_goal_discovery_flow()))
    results.append(("Professions Config", test_professions_have_write_goals()))
    results.append(("Flow Registry", test_flow_registry_has_goal_discovery()))

    print("\n" + "=" * 55)
    print("SUMMARY")
    print("=" * 55)
    for name, ok in results:
        print(f"  {'OK' if ok else 'FAIL'} {name}")

    all_ok = all(r[1] for r in results)
    sys.exit(0 if all_ok else 1)


if __name__ == "__main__":
    main()
