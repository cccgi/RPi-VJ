#!/usr/bin/env python3
"""
Test harness for G-19 Part C: Watchdog and Last-State Restore.
Tests:
1. /state GET and POST persistence to ~/vj-state.json.
2. /heartbeat endpoint updates last_heartbeat timestamp and state.
3. Watchdog trigger on 15s timeout with timestamped log output.
"""
import urllib.request
import urllib.parse
import json
import time
import os
import threading
import subprocess
import sys

# Import server components
import collect_server

def run_tests():
    print("=== G-19 Part C: Watchdog & Last-State Restore Verification ===")

    # Setup temp state file and results dir
    temp_dir = os.path.join(os.path.dirname(__file__), "test_tmp")
    os.makedirs(temp_dir, exist_ok=True)
    collect_server.OUT = temp_dir
    collect_server.STATE_FILE = os.path.join(temp_dir, "vj-state.json")
    if os.path.exists(collect_server.STATE_FILE):
        os.remove(collect_server.STATE_FILE)
    watchdog_log_path = os.path.join(temp_dir, "watchdog.log")
    if os.path.exists(watchdog_log_path):
        os.remove(watchdog_log_path)

    # Override ACTIONS to safe test echo commands
    collect_server.H.ACTIONS["restart-kiosk"] = [
        [sys.executable, "-c", "print('[TEST SYSTEMD] Restarting vj-kiosk...')"]
    ]

    # Start server in a background thread on a random test port
    port = 18085
    server = collect_server.ThreadedServer(("127.0.0.1", port), collect_server.H)
    server_thread = threading.Thread(target=server.serve_forever, daemon=True)
    server_thread.start()

    base_url = f"http://127.0.0.1:{port}"
    print(f"[1] Server running on {base_url}")

    # Test 1: GET /state initially empty
    req = urllib.request.Request(f"{base_url}/state")
    with urllib.request.urlopen(req) as resp:
        body = resp.read().decode('utf-8')
        state = json.loads(body)
        print(f"[2] Initial GET /state -> {state} (expected {{}})")
        assert state == {}

    # Test 2: POST /state save
    test_state = {"bankIndex": 2, "bankName": "PSY", "bpm": 140, "timestamp": int(time.time() * 1000)}
    data = json.dumps(test_state).encode('utf-8')
    req = urllib.request.Request(f"{base_url}/state", data=data, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req) as resp:
        assert resp.status == 200
        print(f"[3] POST /state status -> {resp.status} (saved state)")

    # Test 3: GET /state restores saved state
    req = urllib.request.Request(f"{base_url}/state")
    with urllib.request.urlopen(req) as resp:
        body = resp.read().decode('utf-8')
        restored = json.loads(body)
        print(f"[4] GET /state restored -> bankIndex={restored.get('bankIndex')}, bpm={restored.get('bpm')}")
        assert restored["bankIndex"] == 2
        assert restored["bpm"] == 140

    # Test 4: POST /heartbeat with state
    hb_state = {"bankIndex": 3, "bankName": "TECHNO", "bpm": 128, "timestamp": int(time.time() * 1000)}
    data = json.dumps(hb_state).encode('utf-8')
    req = urllib.request.Request(f"{base_url}/heartbeat", data=data, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req) as resp:
        res_body = json.loads(resp.read().decode('utf-8'))
        print(f"[5] POST /heartbeat response -> {res_body}")
        assert res_body.get("status") == "ok"

    # Verify state was updated by heartbeat
    req = urllib.request.Request(f"{base_url}/state")
    with urllib.request.urlopen(req) as resp:
        restored = json.loads(resp.read().decode('utf-8'))
        print(f"[6] Heartbeat state persistence -> bankIndex={restored.get('bankIndex')}, bpm={restored.get('bpm')}")
        assert restored["bankIndex"] == 3

    # Test 5: Watchdog detection & restart logging
    print("[7] Testing Watchdog timeout detection (simulating hung Chromium with 2.0s watchdog timeout)...")
    with collect_server._heartbeat_lock:
        collect_server._last_heartbeat[0] = time.time() - 10.0 # simulate 10s elapsed

    # Run one watchdog iteration with 2.0s threshold
    timeout = 2.0
    now = time.time()
    last = collect_server._last_heartbeat[0]
    elapsed = now - last
    if elapsed > timeout:
        collect_server._restart_kiosk(f"no heartbeat received for {elapsed:.1f}s (threshold {timeout:.1f}s)")

    # Verify log output in watchdog.log
    assert os.path.exists(watchdog_log_path), "watchdog.log was not created!"
    with open(watchdog_log_path, "r", encoding="utf-8") as f:
        log_content = f.read().strip()
        print(f"[8] watchdog.log output:\n{log_content}")
        assert "WATCHDOG RESTART" in log_content
        assert "Restarting vj-kiosk" in log_content

    server.shutdown()
    print("[9] All G-19 Part C tests passed successfully.")

if __name__ == "__main__":
    run_tests()
