#!/usr/bin/env bash
# Share the project over SMB so it can be edited directly from Windows.
# Run ON THE PI as rpi (it calls sudo itself):
#     ssh -t rpi@rpi-vj.local "cd ~/RPi-VJ && ./gig/deploy/install-samba.sh"
set -euo pipefail
[ "$(id -u)" -ne 0 ] || { echo "!! run as rpi, not root"; exit 1; }

REPO="$(cd "$(dirname "$0")/../.." && pwd)"
USER_NAME="$USER"
SHARE="RPi-VJ"

echo "== installing samba (and wsdd so Windows can discover the Pi)"
sudo apt-get update -qq
sudo apt-get install -y samba samba-common-bin
sudo apt-get install -y wsdd 2>/dev/null || echo "   (wsdd unavailable — use \\\\rpi-vj.local directly)"

CONF=/etc/samba/smb.conf
if grep -q "^\[$SHARE\]" "$CONF" 2>/dev/null; then
  echo "== share [$SHARE] already present"
else
  echo "== adding share [$SHARE] -> $REPO"
  sudo tee -a "$CONF" >/dev/null <<EOF

[$SHARE]
   comment = RPi-VJ project
   path = $REPO
   browseable = yes
   read only = no
   guest ok = no
   valid users = $USER_NAME
   force user = $USER_NAME
   create mask = 0664
   directory mask = 0775
   # Keep shell scripts executable when written from Windows (NTFS has no
   # exec bit, which is why scp kept dropping it).
   force create mode = 0664
   veto files = /.DS_Store/Thumbs.db/
   delete veto files = yes
EOF
fi

echo
echo "== set the SMB password for '$USER_NAME' (this is SEPARATE from the"
echo "   Linux login password — Samba keeps its own database)"
sudo smbpasswd -a "$USER_NAME"
sudo smbpasswd -e "$USER_NAME" >/dev/null 2>&1 || true

sudo testparm -s >/dev/null 2>&1 && echo "== smb.conf validates" || { echo "!! smb.conf INVALID"; exit 1; }
sudo systemctl restart smbd nmbd 2>/dev/null || sudo systemctl restart smbd
sudo systemctl enable smbd >/dev/null 2>&1 || true
sudo systemctl enable wsdd >/dev/null 2>&1 || true
sudo systemctl start wsdd 2>/dev/null || true

IP="$(hostname -I | awk '{print $1}')"
cat <<EOF

=========================================================================
 SMB share is live.

   From Windows Explorer:   \\\\rpi-vj.local\\$SHARE
   or by IP:                \\\\$IP\\$SHARE
   username: $USER_NAME     password: (the SMB one you just set)

   Map a drive (PowerShell):
     net use V: \\\\rpi-vj.local\\$SHARE /user:$USER_NAME /persistent:yes

 IMPORTANT: editing files over SMB does NOT restart the kiosk. After a
 change, reload with:
     ssh rpi@rpi-vj.local "systemctl --user restart vj-kiosk"
 or run  .\\gig\\tools\\reload.ps1  from the repo on Windows.
=========================================================================
EOF
