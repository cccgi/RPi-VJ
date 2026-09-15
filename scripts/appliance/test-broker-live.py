import socket
import json
import urllib.request
import time

print("1. Testing Unix Domain Socket (/run/rpi-vj/broker.sock)...")
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.connect("/run/rpi-vj/broker.sock")

cmd = {"v": 1, "id": "test-uuid-live-42", "type": "command", "name": "clip.trigger", "args": {"key": 7, "bank": 3}}
s.sendall((json.dumps(cmd) + "\n").encode("utf-8"))
resp_line = s.recv(1024).decode("utf-8")
print("Response from Unix socket:", resp_line.strip())
ack = json.loads(resp_line)
assert ack["status"] == "accepted", f"Expected accepted, got: {ack}"

print("\n2. Querying HTTP API (/api/v1/state)...")
req = urllib.request.urlopen("http://127.0.0.1:8080/api/v1/state")
state = json.loads(req.read().decode("utf-8"))
print("Current show state:", json.dumps(state))
assert state["active_clip"] == 7
assert state["active_bank"] == 3

print("\n3. Testing Command Deduplication...")
s.sendall((json.dumps(cmd) + "\n").encode("utf-8"))
resp_line2 = s.recv(1024).decode("utf-8")
print("Dedup Response:", resp_line2.strip())
ack2 = json.loads(resp_line2)
assert ack2["status"] == "accepted"

print("\n4. Testing HTTP status endpoint...")
req_status = urllib.request.urlopen("http://127.0.0.1:8080/api/v1/status")
status_data = json.loads(req_status.read().decode("utf-8"))
print("Supervisor status:", json.dumps(status_data))

print("\n=== ALL LIVE HARDWARE BROKER TESTS PASSED ===")
