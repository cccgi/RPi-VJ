#!/usr/bin/env python3
"""Drive the kiosk page over CDP from the Pi itself. No tunnel, no PC browser.

  python3 cdp_freeze_test.py "window.lastFrameAt"
  python3 cdp_freeze_test.py "cancelAnimationFrame(mixer._rafId)"

Chromium must be started with --remote-debugging-port=9222 (it binds IPv4 only,
so always address it as 127.0.0.1, never localhost).
"""
import json, sys, urllib.request

PORT = 9222

try:
    from websocket import create_connection
except ImportError:
    sys.exit("missing dep:  pip3 install --break-system-packages websocket-client")

try:
    targets = json.load(urllib.request.urlopen("http://127.0.0.1:%d/json" % PORT, timeout=5))
except Exception as e:
    sys.exit("cannot reach CDP on 127.0.0.1:%d (%s) -- is the kiosk up with the flag?" % (PORT, e))

pages = [t for t in targets if t.get("type") == "page" and "/gig/" in t.get("url", "")]
if not pages:
    sys.exit("no /gig/ page target found. targets: %s"
             % [t.get("url") for t in targets])

expr = sys.argv[1] if len(sys.argv) > 1 else "window.lastFrameAt"
ws = create_connection(pages[0]["webSocketDebuggerUrl"], timeout=10)
ws.send(json.dumps({"id": 1, "method": "Runtime.evaluate",
                    "params": {"expression": expr, "returnByValue": True,
                               "awaitPromise": True}}))
resp = json.loads(ws.recv())
ws.close()

r = resp.get("result", {})
if "exceptionDetails" in r:
    print("EXCEPTION:", r["exceptionDetails"].get("text"),
          r["exceptionDetails"].get("exception", {}).get("description", ""))
    sys.exit(1)
print(json.dumps(r.get("result", r), indent=2))
