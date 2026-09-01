# A3-05 — Command Broker and Monotonic Supervisor

## Goal

Supervise the complete appliance, including failures before the first browser
heartbeat, and provide one command/state contract to controllers and renderers.

## Scope

Implement a local Unix-socket broker with versioned JSON messages:

```json
{"v":1,"id":"uuid","type":"command","name":"clip.trigger","args":{"key":7}}
{"v":1,"id":"uuid","type":"ack","status":"accepted|applied|failed","detail":"..."}
```

Persist only desired show state, not transient button edges. Deduplicate command
IDs. Controller acceptance must never wait for renderer or icon I/O.

The supervisor observes renderer unit state, compositor/DRM readiness, first
frame, frame heartbeat, controller state, disk reserve, and restart frequency.
Use monotonic deadlines and an escalation ladder: restart renderer cgroup, then
one controlled reboot, then durable degraded state. Apply backoff and prevent
reboot loops.

## Observable

With the renderer stopped, submit a clip command and receive `accepted`. Start
the renderer and receive/apparently apply the desired state. Kill Cage and
Chromium separately; each recovers. Jump wall time; no false recovery occurs.

