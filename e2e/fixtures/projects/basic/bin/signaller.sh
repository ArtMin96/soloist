#!/usr/bin/env bash
# A long-lived stub that raises real terminal signals on cue: a bare BEL when `cue-bell` appears,
# and a libnotify-compatible OSC 777 notification when `cue-notify` does. The two cues are
# independent and each fires once, so a spec drives exactly the signal it asserts on — a stub
# emitting both at once would raise two alerts and put a second toast and a second unread in front
# of every count. Cued for the same reason cued-crasher.sh is.
set -euo pipefail

echo "signaller ready"
bell=false
notify=false
while true; do
  if [ "$bell" = false ] && [ -e cue-bell ]; then
    bell=true
    printf '\a'
  fi
  if [ "$notify" = false ] && [ -e cue-notify ]; then
    notify=true
    printf '\033]777;notify;Build;done\a'
  fi
  sleep 0.1
done
