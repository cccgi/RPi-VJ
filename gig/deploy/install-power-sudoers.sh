#!/usr/bin/env bash
# Allow the shutdown/reboot the Stream Deck SYS key needs — and nothing else.
# Three exact commands, no wildcards. Run ON THE PI.
set -euo pipefail
F=/etc/sudoers.d/vj-power
sudo tee "$F" >/dev/null <<EOF
${SUDO_USER:-$USER} ALL=(root) NOPASSWD: /sbin/shutdown, /sbin/reboot, /usr/sbin/shutdown, /usr/sbin/reboot
EOF
sudo chmod 0440 "$F"
if sudo visudo -c >/dev/null 2>&1; then
  echo "== installed and validated: $F"
  sudo -n /sbin/shutdown --help >/dev/null 2>&1 && echo "== passwordless shutdown OK" || echo "!! still prompting — check paths"
else
  echo "!! sudoers validation FAILED — removing"; sudo rm -f "$F"; exit 1
fi
