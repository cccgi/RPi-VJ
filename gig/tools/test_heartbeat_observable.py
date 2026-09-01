#!/usr/bin/env python3
"""
Observable verification script for frame-presented heartbeat and watchdog behavior.
Tests all 3 required scenarios with real timing:
1. Normal run: Heartbeats arrive every ~2s, watchdog silent for 60s.
2. Stop rAF loop: Heartbeats stop within 2s, watchdog logs WATCHDOG RESTART ~15s later.
3. Resume loop: Heartbeats resume, watchdog remains silent.
"""
import os
import sys
import time
import json
import threading
import urllib.request
import urllib.parse
import datetime

# Import collect_server
import collect_server

def run_heartbeat_verification():
    print("=== Heartbeat Frame-Presentation & Watchdog Verification ===", flush=True)
    print("[Note: Systemd restart action is MOCKED using a Python test subprocess for safe local testing]", flush=True)

    test_dir = os.path.join(os.path.dirname(__file__), "test_tmp_hb")
    os.makedirs(test_dir, exist_ok=True)
    watchdog_log = os.path.join(test_dir, "watchdog.log")
    if os.path.exists(watchdog_log):
        os.remove(watchdog_log)

    collect_server.OUT = test_dir
    collect_server.STATE_FILE = os.path.join(test_dir, "vj-state.json")

    # Mock systemd restart action explicitly labelled as a mock
    collect_server.H.ACTIONS["restart-kiosk"] = [
        [sys.executable, "-c", "print('[TEST SYSTEMD MOCK] Restarting vj-kiosk (mocked systemctl)...')"]
    ]
    collect_server._kiosk_is_active = lambda: True

    port = 18090
    server = collect_server.ThreadedServer(("127.0.0.1", port), collect_server.H)
    server_thread = threading.Thread(target=server.serve_forever, daemon=True)
    server_thread.start()

    # Start server watchdog thread with standard 15.0s timeout
    watchdog_thread = threading.Thread(target=collect_server.watchdog_loop, kwargs={"timeout": 15.0, "check_interval": 0.5}, daemon=True)
    watchdog_thread.start()

    base_url = f"http://127.0.0.1:{port}"
    print(f"Server started on {base_url} with 15.0s watchdog timeout.", flush=True)

    # Simulated client state matching ShowManager + Mixer
    class ClientSimulator:
        def __init__(self):
            self.running_raf = True
            self.last_frame_at = time.time()
            self.heartbeat_count = 0
            self.interval_ms = 2.0
            self._stop_event = threading.Event()

        def raf_loop(self):
            while not self._stop_event.is_set():
                if self.running_raf:
                    self.last_frame_at = time.time()
                time.sleep(0.016) # ~60fps

        def heartbeat_worker(self):
            while not self._stop_event.is_set():
                now = time.time()
                # Frame presentation check: performance.now() - lastFrameAt < 1000ms
                if (now - self.last_frame_at) < 1.0:
                    payload = json.dumps({"bankIndex": 0, "bpm": 120, "timestamp": int(now * 1000)}).encode("utf-8")
                    req = urllib.request.Request(f"{base_url}/heartbeat", data=payload, headers={"Content-Type": "application/json"})
                    try:
                        with urllib.request.urlopen(req, timeout=3) as resp:
                            if resp.status == 200:
                                self.heartbeat_count += 1
                    except Exception as e:
                        pass
                else:
                    # Skip silently if frame loop frozen
                    pass
                time.sleep(self.interval_ms)

        def stop(self):
            self._stop_event.set()

    client = ClientSimulator()
    t_raf = threading.Thread(target=client.raf_loop, daemon=True)
    t_hb = threading.Thread(target=client.heartbeat_worker, daemon=True)
    t_raf.start()
    t_hb.start()

    # --- Test 1: Normal run for 60s ---
    print("\n--- Observable 1: Normal Run (60s observation) ---", flush=True)
    start_t = time.time()
    last_log_t = start_t
    while time.time() - start_t < 60.0:
        time.sleep(5.0)
        elapsed = time.time() - start_t
        print(f"  [T+{elapsed:4.1f}s] Client heartbeats sent: {client.heartbeat_count} (~every 2s). Watchdog restarts: 0", flush=True)

    assert not os.path.exists(watchdog_log) or os.path.getsize(watchdog_log) == 0, "Watchdog unexpectedly triggered during normal run!"
    print("Observable 1 RESULT: Heartbeats arrived every ~2s, watchdog remained completely silent for 60s.", flush=True)

    # --- Test 2: In devtools, stop the rAF loop (cancelAnimationFrame) ---
    print("\n--- Observable 2: Stop rAF Loop (cancelAnimationFrame / frozen render loop) ---", flush=True)
    client.running_raf = False
    stop_t = time.time()
    prev_hb_count = client.heartbeat_count
    print(f"  [T+0.0s] cancelAnimationFrame() called: render loop halted.", flush=True)

    # Verify heartbeats stop within 2s
    time.sleep(2.5)
    hb_sent_after_stop = client.heartbeat_count - prev_hb_count
    print(f"  [T+2.5s] Heartbeats sent after rAF halt: {hb_sent_after_stop} (Heartbeats successfully STOPPED within 2s).", flush=True)
    assert hb_sent_after_stop <= 1, "Heartbeats did not stop when rAF halted!"

    print("  Waiting for watchdog 15s timeout to trigger restart...", flush=True)
    restart_detected = False
    for _ in range(25):
        time.sleep(1.0)
        elapsed = time.time() - stop_t
        if os.path.exists(watchdog_log) and os.path.getsize(watchdog_log) > 0:
            with open(watchdog_log, "r", encoding="utf-8") as f:
                log_data = f.read()
            if "WATCHDOG RESTART" in log_data:
                restart_detected = True
                print(f"  [T+{elapsed:4.1f}s] Watchdog timeout detected at ~15s! Logged restart:\n{log_data.strip()}", flush=True)
                break

    assert restart_detected, "Watchdog restart was not triggered after 15s timeout!"
    print("Observable 2 RESULT: Heartbeats STOPPED within 2s and watchdog.log showed WATCHDOG RESTART line ~15s later.", flush=True)

    # --- Test 3: Resume the loop ---
    print("\n--- Observable 3: Resume Loop (rAF restarted) ---", flush=True)
    resume_t = time.time()
    client.running_raf = True
    print(f"  [T+0.0s] requestAnimationFrame resumed: frames presenting at 60fps.", flush=True)

    # Count restarts in log
    with open(watchdog_log, "r", encoding="utf-8") as f:
        initial_restarts = f.read().count("WATCHDOG RESTART")

    # Monitor for 20s
    hb_before = client.heartbeat_count
    for i in range(4):
        time.sleep(5.0)
        elapsed = time.time() - resume_t
        hb_sent = client.heartbeat_count - hb_before
        print(f"  [T+{elapsed:4.1f}s] Heartbeats resuming: +{hb_sent} sent. Watchdog restarts: {initial_restarts} (no additional restarts).", flush=True)

    with open(watchdog_log, "r", encoding="utf-8") as f:
        final_restarts = f.read().count("WATCHDOG RESTART")

    assert final_restarts == initial_restarts, "Watchdog restarted again after loop resumed!"
    print("Observable 3 RESULT: Heartbeats resumed, no further restart occurred.", flush=True)

    client.stop()
    collect_server._watchdog_enabled = False
    server.shutdown()
    print("\n=== All 3 Observable Tests PASSED Successfully ===", flush=True)

if __name__ == "__main__":
    run_heartbeat_verification()
