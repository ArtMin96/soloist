#!/usr/bin/env bash
# A long-lived stub process: prints a ready marker, then echoes each line it is given back with a
# stable prefix. Long-lived is the point — a start/stop spec needs a process that stays up until
# told otherwise; the echo gives a future PTY round-trip spec something deterministic to drive.
set -euo pipefail

echo "echo-loop ready"
while IFS= read -r line; do
  echo "echo: ${line}"
done
