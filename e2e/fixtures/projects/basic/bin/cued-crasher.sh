#!/usr/bin/env bash
# A long-lived stub that exits nonzero only once a `cue-crash` file appears beside the project's
# solo.yml. Cued rather than immediate — unlike crasher.sh — because starting a process from its
# row selects it, and an alert about the process the user is looking at is suppressed by design:
# a crash that must reach the user has to land after the spec has looked somewhere else. Deleting
# the cue re-arms it, so one row can be crashed more than once in a walk.
set -euo pipefail

echo "cued-crasher waiting"
while [ ! -e cue-crash ]; do
  sleep 0.1
done
exit 1
