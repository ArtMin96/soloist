#!/usr/bin/env bash
# A stub process that always fails, on cue and immediately. Specs use it to drive crash handling
# and the restart rate limit deterministically, without waiting on a real process to misbehave.
set -euo pipefail

echo "crasher starting"
exit 1
