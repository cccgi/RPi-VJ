#!/usr/bin/env bash
# SUPERSEDED 2026-08-28. This script used to write or patch systemd units.
#
# Unit installation is now handled by exactly ONE script, because having
# several meant a later run could silently revert an earlier fix -- including
# replacing collect_server.py with a plain file server, which removes the
# watchdog, /heartbeat and /system/shutdown while the show still appears to
# work. Whatever this script did is folded into the canonical units in
# deploy/vj-kiosk.service and deploy/vj-server.service.
#
# The original is kept at deploy/_superseded/unset-cache-flags.sh for reference.
echo "!! unset-cache-flags.sh is superseded."
echo "!! Use:  ./gig/deploy/install-units.sh          (install/refresh)"
echo "!!       ./gig/deploy/install-units.sh --check  (verify only)"
exit 1
