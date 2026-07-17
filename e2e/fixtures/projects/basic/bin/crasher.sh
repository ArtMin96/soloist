#!/usr/bin/env bash
# A stub process that always fails, on cue and immediately, so a spec asserts crash handling
# without waiting on a real process to misbehave.
set -euo pipefail

echo "crasher starting"
exit 1
