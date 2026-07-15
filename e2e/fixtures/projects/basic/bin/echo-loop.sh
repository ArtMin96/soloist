#!/usr/bin/env bash
# A long-lived stub process: prints a ready marker, then echoes each line it is given back with a
# stable prefix. Specs assert on the marker to know it is up, and drive the echo to prove typed
# input reaches the real PTY and its output reaches the terminal.
set -euo pipefail

echo "echo-loop ready"
while IFS= read -r line; do
  echo "echo: ${line}"
done
