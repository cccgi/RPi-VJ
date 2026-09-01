#!/usr/bin/env bash
# Mark the last Chromium session as a clean exit so the "restore session"
# bubble can never appear. A power cut mid-set otherwise leaves an unclickable
# dialog on the TV at the next boot — there is no mouse at the gig.
# Safe to run when Chromium is not running. Idempotent.
set -euo pipefail
for P in "$HOME/.config/chromium/Default/Preferences" \
         "$HOME/snap/chromium/common/chromium/Default/Preferences"; do
  [ -f "$P" ] || continue
  python3 - "$P" <<'PY'
import json,sys
p=sys.argv[1]
try:
    d=json.load(open(p,encoding='utf-8'))
except Exception as e:
    print("  skip %s (%s)"%(p,e)); raise SystemExit
prof=d.setdefault('profile',{})
changed=False
if prof.get('exit_type')!='Normal': prof['exit_type']='Normal'; changed=True
if prof.get('exited_cleanly') is not True: prof['exited_cleanly']=True; changed=True
if changed:
    json.dump(d,open(p,'w',encoding='utf-8'))
    print("  sanitized %s"%p)
else:
    print("  already clean %s"%p)
PY
done
